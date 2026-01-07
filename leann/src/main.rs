use actix_web::{get, post, web, App, HttpResponse, HttpServer, Responder};
use pyo3::prelude::*;
use pyo3::types::{IntoPyDict, PyList}; // Import essentiel pour into_py_dict
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use std::env;

// --- STRUCTURES JSON ---
#[derive(Deserialize)]
struct Document {
    text: String,
}

#[derive(Deserialize)]
struct InsertRequest {
    documents: Vec<Document>,
}

#[derive(Deserialize)]
struct SearchRequest {
    k: usize,
}

#[derive(Serialize)]
struct SearchResult {
    id: String,
    text: String,
    score: f32,
}

// --- ÉTAT GLOBAL ---
struct AppState {
    // Py<PyAny> est un pointeur thread-safe vers un objet Python
    searcher: Mutex<Option<Py<PyAny>>>,
    index_path: String,
}

// --- LOGIQUE PYTHON (PyO3) ---

fn init_python_leann(index_path: &str) -> PyResult<Option<Py<PyAny>>> {
    Python::with_gil(|py| {
        let os = py.import("os")?;
        
        // Vérifie si l'index existe
        let path_exists: bool = os.call_method1("path.exists", (index_path,))?.extract()?;
        
        if path_exists {
            println!("🦀 Rust: Chargement de l'index LEANN depuis {}...", index_path);
            let leann_module = py.import("leann")?;
            let searcher_class = leann_module.getattr("LeannSearcher")?;
            let searcher_instance = searcher_class.call1((index_path,))?;
            
            // On convertit l'objet Python en pointeur persistant
            Ok(Some(searcher_instance.into()))
        } else {
            println!("🦀 Rust: Aucun index trouvé à {}.", index_path);
            Ok(None)
        }
    })
}

fn python_insert(documents: Vec<String>, index_path: &str) -> PyResult<()> {
    Python::with_gil(|py| {
        let leann = py.import("leann")?;
        
        // Correction de la syntaxe des dictionnaires pour PyO3 0.20+
        let config = vec![
            ("backend_name", "hnsw"),
            ("embedding_mode", "sentence-transformers"),
            ("embedding_model", "all-MiniLM-L6-v2")
        ].into_py_dict(py); // Nécessite 'use pyo3::types::IntoPyDict;'

        let builder = leann.call_method(
            "LeannBuilder", 
            (), 
            Some(config)
        )?;

        for text in documents {
            builder.call_method1("add_text", (text,))?;
        }

        // Création du dossier si inexistant
        let os = py.import("os")?;
        let path_obj = std::path::Path::new(index_path);
        if let Some(parent) = path_obj.parent() {
            let parent_str = parent.to_str().unwrap_or("/data");
            if !std::path::Path::new(parent_str).exists() {
                 os.call_method1("makedirs", (parent_str,))?;
            }
        }
        
        builder.call_method1("build_index", (index_path,))?;
        Ok(())
    })
}

// Correction majeure ici : on passe une copie du pointeur (Py<PyAny>) 
// et non une référence pour satisfaire le borrow checker.
fn python_search(searcher: Py<PyAny>, k: usize) -> PyResult<Vec<SearchResult>> {
    Python::with_gil(|py| {
        // 'searcher' est un pointeur. On le lie au GIL pour l'utiliser.
        let searcher_bound = searcher.bind(py);
        
        // Appel de la méthode search
        let results_py = searcher_bound.call_method1("search", ("query placeholder", k))?;
        
        let mut results = Vec::new();
        
        // Correction de l'itération pour PyO3 0.20+
        // On tente de traiter le résultat comme une liste
        if let Ok(py_list) = results_py.downcast::<PyList>() {
            for item in py_list {
                // Extraction robuste (si un champ manque, on met une valeur par défaut)
                let text: String = item.getattr("text").ok()
                    .and_then(|v| v.extract().ok())
                    .unwrap_or_else(|| item.to_string());
                
                let id: String = item.getattr("id").ok()
                    .and_then(|v| v.extract().ok())
                    .unwrap_or_else(|| "unknown".to_string());
                
                let score: f32 = item.getattr("score").ok()
                    .and_then(|v| v.extract().ok())
                    .unwrap_or(0.0);
                
                results.push(SearchResult { id, text, score });
            }
        }
        Ok(results)
    })
}

// --- HANDLERS HTTP ACTIX ---

#[get("/health")]
async fn health() -> impl Responder {
    HttpResponse::Ok().json(serde_json::json!({"status": "ok", "engine": "leann-rust-wrapper"}))
}

#[post("/insert")]
async fn insert(req: web::Json<InsertRequest>, data: web::Data<AppState>) -> impl Responder {
    let docs: Vec<String> = req.documents.iter().map(|d| d.text.clone()).collect();
    let count = docs.len();
    let path = data.index_path.clone();

    // web::block exécute le code bloquant (Python) dans un thread séparé
    let res = web::block(move || python_insert(docs, &path)).await;

    match res {
        Ok(Ok(_)) => {
            // Rechargement à chaud (Hot Reload) de l'index
            // On peut faire un unwrap ici car si python_insert a réussi, l'init réussira
            if let Ok(Some(new_searcher)) = init_python_leann(&data.index_path) {
                let mut guard = data.searcher.lock().unwrap();
                *guard = Some(new_searcher);
            }
            
            HttpResponse::Ok().json(serde_json::json!({"status": "indexed", "count": count}))
        },
        Ok(Err(e)) => {
            eprintln!("Erreur Python: {:?}", e);
            HttpResponse::InternalServerError().body(format!("Erreur Python: {}", e))
        },
        Err(e) => HttpResponse::InternalServerError().body(format!("Erreur Thread: {}", e)),
    }
}

#[post("/search")]
async fn search(req: web::Json<SearchRequest>, data: web::Data<AppState>) -> impl Responder {
    // 1. On récupère le pointeur Python de manière thread-safe
    let searcher_opt = {
        let guard = data.searcher.lock().unwrap();
        // CLONE CRUCIAL : On clone le pointeur (Py<PyAny>).
        // Cela incrémente juste le ref-count, c'est très rapide.
        // Cela permet à 'searcher_ptr' de vivre indépendamment de 'data'.
        guard.clone()
    };
    
    if let Some(searcher_ptr) = searcher_opt {
        let k = req.k;
        
        // 2. On envoie le pointeur cloné dans le thread (move transfère la propriété du clone)
        let res = web::block(move || python_search(searcher_ptr, k)).await;
        
        match res {
            Ok(Ok(results)) => HttpResponse::Ok().json(serde_json::json!({"results": results})),
            Ok(Err(e)) => {
                eprintln!("Erreur Recherche Python: {:?}", e);
                HttpResponse::InternalServerError().body(format!("Erreur Recherche: {}", e))
            },
            Err(_) => HttpResponse::InternalServerError().body("Erreur interne serveur"),
        }
    } else {
        HttpResponse::Ok().json(serde_json::json!({"results": [], "warning": "Index vide, insérez des documents d'abord"}))
    }
}

// --- MAIN ---

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Config
    let index_dir = env::var("DATA_DIR").unwrap_or("/data".to_string());
    let index_path = format!("{}/default_index", index_dir);

    println!("🚀 Init Python...");
    let searcher = init_python_leann(&index_path).unwrap_or(None);
    
    let state = web::Data::new(AppState {
        searcher: Mutex::new(searcher),
        index_path,
    });

    println!("🚀 Serveur LEANN (Rust Wrapper) démarré sur 0.0.0.0:8000");

    HttpServer::new(move || {
        App::new()
            .app_data(state.clone())
            .service(health)
            .service(insert)
            .service(search)
    })
    .bind(("0.0.0.0", 8000))?
    .run()
    .await
}