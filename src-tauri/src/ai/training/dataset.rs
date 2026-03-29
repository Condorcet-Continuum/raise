// FICHIER : src-tauri/src/ai/training/dataset.rs

use crate::json_db::collections::manager::CollectionsManager;
use crate::utils::prelude::*;

#[derive(Debug, Serializable, Deserializable, Clone, PartialEq)]
pub struct TrainingExample {
    pub instruction: String,
    pub input: String,
    pub output: String,
}

/// Extrait les données spécifiquement pour un domaine métier à partir du JSON-DB.
/// Cette fonction est utilisée par le moteur d'entraînement natif.
pub async fn extract_domain_data(
    manager: &CollectionsManager<'_>,
    domain: &str,
) -> RaiseResult<Vec<TrainingExample>> {
    let mut dataset = Vec::new();

    let collections = match manager.list_collections().await {
        Ok(c) => c,
        Err(e) => {
            raise_error!(
                "ERR_VECTOR_DB_LIST_FAILED",
                error = e,
                context = json_value!({
                    "action": "list_collections",
                    "storage": "qdrant_internal",
                    "hint": "Impossible de récupérer la liste des collections. Vérifiez que le service de base de données vectorielle est bien démarré."
                })
            )
        }
    };

    for col in collections {
        if !col.contains(domain) && domain != "all" {
            continue;
        }

        let docs = match manager.list_all(&col).await {
            Ok(d) => d,
            Err(e) => {
                raise_error!(
                    "ERR_VECTOR_DB_FETCH_DOCS_FAILED",
                    error = e,
                    context = json_value!({
                        "collection": col,
                        "action": "list_all_documents",
                        "hint": "Échec de la récupération des documents."
                    })
                )
            }
        };

        for doc in docs {
            dataset.push(TrainingExample {
                instruction: format!("Analyser cet élément technique du domaine {}.", domain),
                input: json::serialize_to_string(&doc).unwrap_or_default(),
                output: format!(
                    "L'entité appartient à la collection '{}' dans l'espace projet '{}'.",
                    col, manager.space
                ),
            });
        }
    }

    Ok(dataset)
}

// --- TESTS UNITAIRES ---
#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::testing::AgentDbSandbox;

    #[async_test]
    async fn test_extract_domain_data_filtering() {
        let sandbox = AgentDbSandbox::new().await;
        let space = &sandbox.config.system_domain;
        let db = &sandbox.config.system_db;
        let manager = CollectionsManager::new(&sandbox.db, space, db);

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

        let doc = json_value!({"_id": "1", "content": "test"});
        manager.insert_raw("safety_rules", &doc).await.unwrap();
        manager.insert_raw("general_info", &doc).await.unwrap();

        let results = extract_domain_data(&manager, "safety").await.unwrap();

        assert_eq!(
            results.len(),
            1,
            "Devrait trouver uniquement la collection safety"
        );
        assert!(results[0].instruction.contains("safety"));

        let all_results = extract_domain_data(&manager, "all").await.unwrap();
        assert_eq!(
            all_results.len(),
            2,
            "Devrait trouver toutes les collections"
        );
    }

    #[async_test]
    async fn test_extract_empty_domain() {
        let sandbox = AgentDbSandbox::new().await;
        let manager = CollectionsManager::new(
            &sandbox.db,
            &sandbox.config.system_domain,
            &sandbox.config.system_db,
        );

        let results = extract_domain_data(&manager, "nonexistent").await.unwrap();
        assert!(
            results.is_empty(),
            "Le dataset devrait être vide pour un domaine inconnu"
        );
    }
}
