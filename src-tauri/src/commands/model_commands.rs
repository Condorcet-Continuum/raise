// FICHIER : src-tauri/src/commands/model_commands.rs

use crate::utils::prelude::*;

use crate::json_db::storage::StorageEngine;
use crate::model_engine::loader::ModelLoader;
use crate::model_engine::types::ProjectModel;
use tauri::{command, State};

/// Charge l'intégralité du modèle en mémoire pour analyse.
/// Cette commande est désormais entièrement asynchrone.
#[command]
pub async fn load_project_model(
    storage: State<'_, StorageEngine>,
    space: String,
    db: String,
) -> RaiseResult<ProjectModel> {
    // On instancie le loader
    let loader = ModelLoader::from_engine(storage.inner(), &space, &db);

    // 1. On capture le résultat avec un match explicite
    let full_model = match loader.load_full_model().await {
        Ok(model) => model,
        Err(e) => raise_error!(
            "ERR_MODEL_LOAD_FAIL",
            error = e,
            context = json!({
                "action": "load_full_project_model",
                "source": "model_loader",
                "hint": "Le chargement a échoué. Vérifiez l'intégrité des JSON et les permissions du dossier racine."
            })
        ),
    };

    // 2. On retourne le succès explicitement
    Ok(full_model)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json_db::storage::JsonDbConfig;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_load_project_model_command() {
        // Simulation de l'environnement
        let dir = tempdir().unwrap();
        let config = JsonDbConfig::new(dir.path().to_path_buf());
        let storage = StorageEngine::new(config);

        // Note: Dans un vrai test Tauri, on utiliserait tauri::test::mock_builder
        // Ici on teste la logique de la fonction asynchrone directement
        let space = "test_space".to_string();
        let db = "test_db".to_string();

        // On enveloppe dans State pour simuler Tauri (si test d'intégration)
        // Mais ici on peut tester le loader via la commande en passant les data
        let loader = ModelLoader::from_engine(&storage, &space, &db);
        let result = loader.load_full_model().await;

        assert!(result.is_ok());
        let model = result.unwrap();
        assert_eq!(model.meta.element_count, 0); // Vide par défaut
    }
}
