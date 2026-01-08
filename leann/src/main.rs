#![allow(deprecated)] // Supprime les warnings de PyO3 0.27

use actix_web::{get, post, web, App, HttpResponse, HttpServer, Responder};
use pyo3::prelude::*;
use pyo3::types::{IntoPyDict, PyList};
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
        let os_path = py.import("os.path")?;
        
        // CORRECTION : On vérifie l'existence du fichier .index spécifique
        // car 'index_path' est juste le préfixe (ex: "default_index")
        let actual_file_path = format!("{}.index", index_path);
        let path_exists: bool = os_path.call_method1("exists", (actual_file_path,))?.extract()?;
        
        if path_exists {
            println!("🦀 Rust: Chargement de l'index LEANN depuis {}...", index_path);
            let leann_module = py.import("leann")?;
            let searcher_class = leann_module.getattr("LeannSearcher")?;
            
            // Pour le chargement, on garde 'index_path' (sans extension), 
            // LEANN se débrouille pour trouver les fichiers .index, .json, etc.
            let searcher_instance = searcher_class.call1((index_path,))?;
            
            Ok(Some(searcher_instance.unbind()))
        } else {
            println!("🦀 Rust: Aucun index trouvé à {}.index", index_path);
            Ok(None)
        }
    })
}

fn python_insert(documents: Vec<String>, index_path: &str) -> PyResult<()> {
    Python::with_gil(|py| {
        let leann = py.import("leann")?;
        
        let config = vec![
            ("backend_name", "hnsw"),
            ("embedding_mode", "sentence-transformers"),
            ("embedding_model", "all-MiniLM-L6-v2")
        ].into_py_dict(py)?; 

        let builder = leann.call_method(
            "LeannBuilder", 
            (), 
            Some(&config)
        )?;

        for text in documents {
            builder.call_method1("add_text", (text,))?;
        }

        // Création du dossier : Ici on utilise os.makedirs qui est bien sur 'os'
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

fn python_search(searcher: Py<PyAny>, k: usize) -> PyResult<Vec<SearchResult>> {
    Python::with_gil(|py| {
        let searcher_bound = searcher.bind(py);
        let results_py = searcher_bound.call_method1("search", ("query placeholder", k))?;
        
        let mut results = Vec::new();
        
        if let Ok(py_list) = results_py.downcast::<PyList>() {
            for item in py_list {
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

    let res = web::block(move || python_insert(docs, &path)).await;

    match res {
        Ok(Ok(_)) => {
            // Rechargement à chaud
            // On garde le diagnostic au cas où le reload échoue aussi
            match init_python_leann(&data.index_path) {
                Ok(Some(new_searcher)) => {
                     let mut guard = data.searcher.lock().unwrap();
                     *guard = Some(new_searcher);
                },
                Ok(None) => eprintln!("⚠️ Index rechargé mais vide/introuvable"),
                Err(e) => eprintln!("❌ Erreur rechargement index: {}", e),
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
    let searcher_opt = {
        let guard = data.searcher.lock().unwrap();
        Python::with_gil(|py| {
            guard.as_ref().map(|obj| obj.clone_ref(py))
        })
    };
    
    if let Some(searcher_ptr) = searcher_opt {
        let k = req.k;
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

    // Initialisation avec gestion d'erreur explicite
    println!("🚀 Init Python...");
    let searcher = match init_python_leann(&index_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("❌ CRASH INIT PYTHON : {}", e);
            None
        }
    };
    
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