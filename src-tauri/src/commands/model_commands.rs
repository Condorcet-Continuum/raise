// FICHIER : src-tauri/src/commands/model_commands.rs

use crate::utils::prelude::*;

use crate::json_db::storage::StorageEngine;
use crate::model_engine::loader::ModelLoader;
use crate::model_engine::types::ProjectModel;
use tauri::{command, State};

/// Charge l'intégralité du modèle en mémoire pour analyse.
#[command]
pub async fn load_project_model(
    storage: State<'_, StorageEngine>,
    space: String,
    db: String,
) -> RaiseResult<ProjectModel> {
    let loader = ModelLoader::from_engine(storage.inner(), &space, &db);

    match loader.load_full_model().await {
        Ok(model) => Ok(model),
        Err(e) => raise_error!(
            "ERR_MODEL_LOAD_FAIL",
            error = e,
            context = json_value!({
                "action": "load_full_project_model",
                "space": space,
                "db": db
            })
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json_db::collections::manager::CollectionsManager;
    use crate::utils::testing::AgentDbSandbox;

    /// Helper pour satisfaire l'exigence Data-Driven du Loader
    async fn inject_mock_mapping(manager: &CollectionsManager<'_>) {
        let _ = manager
            .create_collection(
                "configs",
                "db://_system/_system/schemas/v1/db/generic.schema.json",
            )
            .await;
        manager
            .upsert_document(
                "configs",
                json_value!({
                    "_id": "ref:configs:handle:ontological_mapping",
                    "search_spaces": [ { "layer": "oa", "collection": "actors" } ]
                }),
            )
            .await
            .unwrap();
    }

    #[async_test]
    async fn test_load_project_model_command() {
        // 🎯 On utilise AgentDbSandbox pour avoir une infrastructure complète (Config, Logs, FS)
        let sandbox = AgentDbSandbox::new().await;

        // 1. On prépare la DB système avec le mapping requis
        let sys_mgr = CollectionsManager::new(
            &sandbox.db,
            &sandbox.config.system_domain,
            &sandbox.config.system_db,
        );
        inject_mock_mapping(&sys_mgr).await;

        // 2. On instancie le loader via la logique de la commande
        // On utilise les paramètres de la sandbox pour pointer au bon endroit
        let space = sandbox.config.system_domain.clone();
        let db = sandbox.config.system_db.clone();

        let loader = ModelLoader::from_engine(&sandbox.db, &space, &db);

        // 3. Exécution
        let result = loader.load_full_model().await;

        // 4. Vérification
        assert!(
            result.is_ok(),
            "Le chargement devrait réussir avec un mapping présent"
        );
        let model = result.unwrap();

        // Le compte est à 0 car nous n'avons pas injecté d'éléments métier,
        // mais la structure ProjectModel doit être valide.
        assert_eq!(model.meta.element_count, 0);
    }
}
