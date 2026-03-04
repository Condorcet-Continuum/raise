// FICHIER : src-tauri/src/ai/training/dataset.rs

use crate::json_db::collections::manager::CollectionsManager;
use crate::json_db::storage::StorageEngine;
use crate::utils::prelude::*;

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
) -> RaiseResult<Vec<TrainingExample>> {
    let manager = CollectionsManager::new(storage, space, db_name);
    let mut dataset = Vec::new();

    let collections = match manager.list_collections().await {
        Ok(c) => c,
        Err(e) => {
            raise_error!(
                "ERR_VECTOR_DB_LIST_FAILED",
                error = e,
                context = json!({
                    "action": "list_collections",
                    "storage": "qdrant_internal",
                    "hint": "Impossible de récupérer la liste des collections. Vérifiez que le service de base de données vectorielle est bien démarré."
                })
            )
        }
    };

    for col in collections {
        // Logique de filtrage : on cherche le nom du domaine dans le nom de la collection
        // ou on prend tout si le domaine est "all".
        if !col.contains(domain) && domain != "all" {
            continue;
        }

        let docs = match manager.list_all(&col).await {
            Ok(d) => d,
            Err(e) => {
                raise_error!(
                    "ERR_VECTOR_DB_FETCH_DOCS_FAILED",
                    error = e,
                    context = json!({
                        "collection": col,
                        "action": "list_all_documents",
                        "hint": "Échec de la récupération des documents. Vérifiez si la collection n'a pas été supprimée ou renommée."
                    })
                )
            }
        };

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
) -> RaiseResult<Vec<TrainingExample>> {
    // Cette commande permet au frontend de prévisualiser ou d'exporter les données
    // CORRECTION : Ajout de .await car extract_domain_data est désormais async
    extract_domain_data(storage.inner(), &space, &db_name, &domain).await
}

// --- TESTS UNITAIRES ---

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::mock::AgentDbSandbox;

    #[tokio::test] // CORRECTION : Utilisation de tokio pour les tests asynchrones
    async fn test_extract_domain_data_filtering() {
        // A. Setup d'une base de données temporaire 100% isolée en UNE seule ligne !
        let sandbox = AgentDbSandbox::new().await;

        // On crée des raccourcis vers les vrais noms de domaine et de DB de la configuration
        let space = &sandbox.config.system_domain;
        let db = &sandbox.config.system_db;

        let manager = CollectionsManager::new(&sandbox.db, space, db);

        // B. Création de collections (une 'safety' et une 'other')
        manager
            .create_collection(
                "safety_rules",
                "db://_system/_system/schemas/v1/db/generic.schema.json",
            )
            .await
            .unwrap();
        manager
            .create_collection(
                "general_info",
                "db://_system/_system/schemas/v1/db/generic.schema.json",
            )
            .await
            .unwrap();

        let doc = serde_json::json!({"id": "1", "content": "test"});
        manager.insert_raw("safety_rules", &doc).await.unwrap();
        manager.insert_raw("general_info", &doc).await.unwrap();

        // C. Test du filtrage par domaine 'safety'
        let results = extract_domain_data(&sandbox.db, space, db, "safety")
            .await
            .unwrap();

        assert_eq!(
            results.len(),
            1,
            "Devrait trouver uniquement la collection safety"
        );
        assert!(results[0].instruction.contains("safety"));

        // D. Test avec le domaine 'all'
        let all_results = extract_domain_data(&sandbox.db, space, db, "all")
            .await
            .unwrap();

        assert_eq!(
            all_results.len(),
            2,
            "Devrait trouver toutes les collections"
        );
    }

    #[tokio::test]
    async fn test_extract_empty_domain() {
        let sandbox = AgentDbSandbox::new().await;

        // 2. On utilise le moteur et les noms réels configurés par la sandbox
        let results = extract_domain_data(
            &sandbox.db,
            &sandbox.config.system_domain,
            &sandbox.config.system_db,
            "nonexistent",
        )
        .await
        .unwrap();

        // 3. Les assertions restent inchangées
        assert!(
            results.is_empty(),
            "Le dataset devrait être vide pour un domaine inconnu"
        );
    }
}
