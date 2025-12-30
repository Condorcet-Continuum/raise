use serde_json::json;
use std::env;
use std::fs::{self, File};
use std::io::Write;
use std::path::PathBuf;
use tauri::State;

use crate::json_db::collections::manager::CollectionsManager;
use crate::json_db::storage::StorageEngine;

// --- 1. La Commande Tauri (Le Wrapper) ---
#[tauri::command]
pub async fn ai_export_dataset(
    storage: State<'_, StorageEngine>,
    output_path: String,
    space: String,
    db_name: String,
) -> Result<String, String> {
    // On appelle la logique pure en passant la référence interne (&StorageEngine)
    // Cela permet de tester la logique sans avoir besoin de mocker l'objet "State" de Tauri
    export_logic(storage.inner(), output_path, space, db_name)
}

// --- 2. La Logique Métier (Testable) ---
pub fn export_logic(
    storage: &StorageEngine,
    output_path: String,
    space: String,
    db_name: String,
) -> Result<String, String> {
    println!(
        "🦀 Rust : Export Logic (Space: {}, DB: {})...",
        space, db_name
    );

    let manager = CollectionsManager::new(storage, &space, &db_name);

    // Récupération de la variable d'env (Mockable via std::env::set_var dans les tests)
    let env_path_str = env::var("PATH_GENAPTITUDE_DATASET")
        .unwrap_or_else(|_| "~/genaptitude_dataset".to_string());

    let base_path = if env_path_str.starts_with("~/") {
        let home = env::var("HOME").map_err(|_| "Impossible de trouver $HOME".to_string())?;
        PathBuf::from(env_path_str.replace("~", &home))
    } else {
        PathBuf::from(&env_path_str)
    };

    let training_dir = base_path.join("training");
    if !training_dir.exists() {
        fs::create_dir_all(&training_dir)
            .map_err(|e| format!("Erreur création dossier training : {}", e))?;
    }
    let full_file_path = training_dir.join(&output_path);

    let mut file = File::create(&full_file_path)
        .map_err(|e| format!("Impossible de créer le fichier de sortie : {}", e))?;

    let collections = manager
        .list_collections()
        .map_err(|e| format!("Erreur listing collections : {}", e))?;

    let mut count = 0;

    for col_name in collections {
        if col_name.starts_with('_') {
            continue;
        }

        let documents = manager
            .list_all(&col_name)
            .map_err(|e| format!("Erreur lecture collection {}: {}", col_name, e))?;

        for doc in documents {
            let doc_id = doc.get("id").and_then(|v| v.as_str()).unwrap_or("unknown");

            let doc_name = doc
                .get("name")
                .or(doc.get("label"))
                .or(doc.get("title"))
                .and_then(|v| v.as_str())
                .unwrap_or(doc_id);

            let training_entry = json!({
                "instruction": format!("Voici un objet technique de type '{}' provenant de la base Arcadia. Analyse sa structure.", col_name),
                "input": serde_json::to_string(&doc).unwrap_or_default(),
                "output": format!("Ceci est l'entité '{}' (ID: {}). Elle est définie dans la collection '{}' du projet '{}'.",
                    doc_name, doc_id, col_name, space)
            });

            if let Err(e) = writeln!(file, "{}", training_entry.to_string()) {
                eprintln!("❌ Erreur écriture ligne : {}", e);
            }
            count += 1;
        }
    }

    Ok(format!(
        "Export terminé ! {} documents extraits vers : {}",
        count,
        full_file_path.to_string_lossy()
    ))
}

// --- 3. Les Tests Unitaires ---
#[cfg(test)]
mod tests {
    use super::*;
    use crate::json_db::storage::JsonDbConfig;
    use serde_json::json;
    use std::io::BufRead;
    use tempfile::tempdir;

    // Helper pour créer un moteur de stockage temporaire
    fn create_test_storage() -> (StorageEngine, tempfile::TempDir) {
        let temp_dir = tempdir().expect("Impossible de créer dossier temp DB");
        let config = JsonDbConfig::new(temp_dir.path().to_path_buf());
        let storage = StorageEngine::new(config);
        (storage, temp_dir)
    }

    #[test]
    fn test_export_dataset_nominal_case() {
        // A. SETUP : Création d'une DB virtuelle
        let (storage, _db_dir) = create_test_storage();
        let space = "test_space";
        let db = "test_db";
        let col = "robots";

        // Insertion de données via le Manager
        let manager = CollectionsManager::new(&storage, space, db);
        manager
            .create_collection(col, None)
            .expect("Création collection failed");

        let doc1 = json!({
            "id": "r1",
            "name": "R2D2",
            "type": "astromech"
        });
        manager.insert_raw(col, &doc1).expect("Insert failed");

        // B. SETUP : Dossier de sortie temporaire (pour simuler ~/genaptitude_dataset)
        let dataset_dir = tempdir().expect("Impossible de créer dossier dataset");
        // On force la variable d'env pour le test
        env::set_var("PATH_GENAPTITUDE_DATASET", dataset_dir.path());

        // C. EXECUTION : Lancement de l'export
        let output_filename = "test_dataset.jsonl";
        let result = export_logic(
            &storage,
            output_filename.to_string(),
            space.to_string(),
            db.to_string(),
        );

        // D. ASSERTIONS
        assert!(result.is_ok(), "L'export a échoué : {:?}", result.err());

        // Vérification du fichier généré
        let expected_path = dataset_dir.path().join("training").join(output_filename);
        assert!(expected_path.exists(), "Le fichier JSONL n'a pas été créé");

        // Lecture du contenu
        let file = File::open(expected_path).expect("Impossible d'ouvrir le fichier généré");
        let lines: Vec<String> = std::io::BufReader::new(file)
            .lines()
            .collect::<Result<_, _>>()
            .unwrap();

        assert_eq!(lines.len(), 1, "Il devrait y avoir 1 ligne dans le dataset");

        let first_line: serde_json::Value =
            serde_json::from_str(&lines[0]).expect("JSONL invalide");

        // Vérification du contenu sémantique
        assert!(first_line["instruction"]
            .as_str()
            .unwrap()
            .contains("robots"));
        assert!(first_line["output"].as_str().unwrap().contains("R2D2"));
        assert!(first_line["output"]
            .as_str()
            .unwrap()
            .contains("test_space"));
    }

    #[test]
    fn test_export_empty_db() {
        // A. Setup Vide
        let (storage, _db_dir) = create_test_storage();
        let dataset_dir = tempdir().expect("dataset temp dir");
        env::set_var("PATH_GENAPTITUDE_DATASET", dataset_dir.path());

        // B. Execution
        let result = export_logic(
            &storage,
            "empty.jsonl".to_string(),
            "vide".to_string(),
            "db".to_string(),
        );

        // C. Assertion
        assert!(result.is_ok());
        let msg = result.unwrap();
        assert!(
            msg.contains("0 documents"),
            "Le message devrait indiquer 0 documents"
        );

        // Le fichier doit quand même exister (mais vide ou juste créé)
        let expected_path = dataset_dir.path().join("training").join("empty.jsonl");
        assert!(expected_path.exists());

        let metadata = fs::metadata(expected_path).unwrap();
        assert_eq!(metadata.len(), 0, "Le fichier devrait être vide");
    }
}
