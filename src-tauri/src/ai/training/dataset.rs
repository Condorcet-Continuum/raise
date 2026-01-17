// FICHIER : src-tauri/src/ai/training/dataset.rs

use crate::json_db::collections::manager::CollectionsManager;
use crate::json_db::storage::StorageEngine;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct TrainingExample {
    pub instruction: String,
    pub input: String,
    pub output: String,
}

/// Extrait les données spécifiquement pour un domaine métier à partir du JSON-DB.
/// Cette fonction est utilisée par le moteur d'entraînement natif.
pub async fn extract_domain_data(
    storage: &StorageEngine,
    space: &str,
    db_name: &str,
    domain: &str,
) -> Result<Vec<TrainingExample>, String> {
    let manager = CollectionsManager::new(storage, space, db_name);
    let mut dataset = Vec::new();

    // CORRECTION E0599 : Ajout de .await car list_collections est asynchrone
    let collections = manager
        .list_collections()
        .await
        .map_err(|e| e.to_string())?;

    for col in collections {
        // Logique de filtrage : on cherche le nom du domaine dans le nom de la collection
        // ou on prend tout si le domaine est "all".
        if !col.contains(domain) && domain != "all" {
            continue;
        }

        // CORRECTION E0599 : Ajout de .await car list_all est asynchrone
        let docs = manager.list_all(&col).await.map_err(|e| e.to_string())?;

        for doc in docs {
            // Construction de l'exemple d'entraînement structuré
            dataset.push(TrainingExample {
                instruction: format!("Analyser cet élément technique du domaine {}.", domain),
                input: serde_json::to_string(&doc).unwrap_or_default(),
                output: format!(
                    "L'entité appartient à la collection '{}' dans l'espace projet '{}'.",
                    col, space
                ),
            });
        }
    }

    Ok(dataset)
}

// --- COMMANDES TAURI ---

#[tauri::command]
pub async fn ai_export_dataset(
    storage: tauri::State<'_, StorageEngine>,
    space: String,
    db_name: String,
    domain: String,
) -> Result<Vec<TrainingExample>, String> {
    // Cette commande permet au frontend de prévisualiser ou d'exporter les données
    // CORRECTION : Ajout de .await car extract_domain_data est désormais async
    extract_domain_data(storage.inner(), &space, &db_name, &domain).await
}

// --- TESTS UNITAIRES ---

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json_db::storage::JsonDbConfig;
    use serde_json::json;
    use tempfile::tempdir;

    #[tokio::test] // CORRECTION : Utilisation de tokio pour les tests asynchrones
    async fn test_extract_domain_data_filtering() {
        // A. Setup d'une base de données temporaire
        let temp_dir = tempdir().expect("Échec création dossier temp");
        let config = JsonDbConfig::new(temp_dir.path().to_path_buf());
        let storage = StorageEngine::new(config);

        let space = "test_space";
        let db = "test_db";
        let manager = CollectionsManager::new(&storage, space, db);

        // B. Création de collections (une 'safety' et une 'other')
        // Correction : await sur les opérations DB si elles sont async
        manager
            .create_collection("safety_rules", None)
            .await
            .unwrap();
        manager
            .create_collection("general_info", None)
            .await
            .unwrap();

        let doc = json!({"id": "1", "content": "test"});
        manager.insert_raw("safety_rules", &doc).await.unwrap();
        manager.insert_raw("general_info", &doc).await.unwrap();

        // C. Test du filtrage par domaine 'safety'
        let results = extract_domain_data(&storage, space, db, "safety")
            .await
            .unwrap();
        assert_eq!(
            results.len(),
            1,
            "Devrait trouver uniquement la collection safety"
        );
        assert!(results[0].instruction.contains("safety"));

        // D. Test avec le domaine 'all'
        let all_results = extract_domain_data(&storage, space, db, "all")
            .await
            .unwrap();
        assert_eq!(
            all_results.len(),
            2,
            "Devrait trouver toutes les collections"
        );
    }

    #[tokio::test] // CORRECTION : Passage en test asynchrone
    async fn test_extract_empty_domain() {
        let temp_dir = tempdir().unwrap();
        let storage = StorageEngine::new(JsonDbConfig::new(temp_dir.path().to_path_buf()));

        let results = extract_domain_data(&storage, "space", "db", "nonexistent")
            .await
            .unwrap();
        assert!(
            results.is_empty(),
            "Le dataset devrait être vide pour un domaine inconnu"
        );
    }
}
