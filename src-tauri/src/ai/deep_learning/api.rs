// FICHIER : src-tauri/src/ai/deep_learning/api.rs

use crate::ai::deep_learning::{
    models::sequence_net::SequenceNet, serialization, trainer::Trainer,
};
use crate::json_db::collections::manager::CollectionsManager;
use crate::json_db::query::{Condition, FilterOperator, Query, QueryEngine, QueryFilter};
use crate::utils::prelude::*;
use candle_core::{DType, Tensor};
use candle_nn::{VarBuilder, VarMap};

/// 🔍 Fonction interne : Récupère les métadonnées depuis JSON-DB et résout dynamiquement le chemin des tenseurs.
async fn fetch_model_metadata(
    manager: &CollectionsManager<'_>,
    domain: &str,
    db: &str,
    urn: &str,
) -> RaiseResult<(DeepLearningConfig, PathBuf, VarMap)> {
    // 1. Décodage de l'URN (ex: ref:dl_models:handle:routing_v1)
    let (col, field, val) = if urn.starts_with("ref:") {
        let parts: Vec<&str> = urn.splitn(4, ':').collect();
        if parts.len() == 4 {
            (
                parts[1].to_string(),
                parts[2].to_string(),
                parts[3].to_string(),
            )
        } else {
            raise_error!(
                "ERR_URN_INVALID",
                error = "Format attendu: ref:collection:champ:valeur"
            );
        }
    } else {
        raise_error!("ERR_URN_MISSING", error = "L'ID doit être une URN valide");
    };

    // 2. Requête dans le Graphe de Connaissances
    let doc = if field == "_id" {
        match manager.get_document(&col, &val).await {
            Ok(Some(d)) => d,
            _ => raise_error!("ERR_GRAPH_MODEL_NOT_FOUND", error = format!("ID: {}", val)),
        }
    } else {
        let mut query = Query::new(&col);
        query.filter = Some(QueryFilter {
            operator: FilterOperator::And,
            conditions: vec![Condition::eq(&field, json_value!(val))],
        });
        query.limit = Some(1);
        let engine = QueryEngine::new(manager);
        match engine.execute_query(query).await {
            Ok(res) if !res.documents.is_empty() => res.documents[0].clone(),
            _ => raise_error!(
                "ERR_GRAPH_MODEL_NOT_FOUND",
                error = format!("{} = {}", field, val)
            ),
        }
    };

    // 3. Extraction de l'ID unique et des hyperparamètres
    let model_id = match doc["_id"].as_str() {
        Some(id) => id,
        None => raise_error!(
            "ERR_GRAPH_MODEL_CORRUPTED",
            error = "Document JSON sans _id"
        ),
    };

    let hp = &doc["hyperparameters"];
    let input_size = hp["input_size"].as_u64().unwrap_or(0) as usize;
    let hidden_size = hp["hidden_size"].as_u64().unwrap_or(0) as usize;
    let output_size = hp["output_size"].as_u64().unwrap_or(0) as usize;
    let learning_rate = hp["learning_rate"].as_f64().unwrap_or(0.001);

    if input_size == 0 || hidden_size == 0 || output_size == 0 {
        raise_error!(
            "ERR_GRAPH_MODEL_CORRUPTED",
            error = "Paramètres nuls ou absents"
        );
    }

    let mut config = AppConfig::get().deep_learning.clone();
    config.input_size = input_size;
    config.hidden_size = hidden_size;
    config.output_size = output_size;
    config.learning_rate = learning_rate;

    // 4. 🎯 RÉSOLUTION DÉTERMINISTE DU CHEMIN (Le miroir parfait du json_db)
    // Structure : PATH_RAISE_DOMAIN / domain / db / tensors / collection / <_id>.safetensors
    let domain_root = AppConfig::get().get_path("PATH_RAISE_DOMAIN").unwrap();
    let weights_path = domain_root
        .join(domain)
        .join(db)
        .join("tensors")
        .join(&col)
        .join(format!("{}.safetensors", model_id));

    // 5. ZÉRO SETUP : Auto-création du fichier binaire s'il est vierge
    if let Some(parent) = weights_path.parent() {
        fs::ensure_dir_async(parent).await?; // 🎯 FIX : On propage l'erreur si la création échoue
    }

    let mut varmap = VarMap::new();
    let device = config.to_device();

    if fs::exists_async(&weights_path).await {
        if let Err(e) = serialization::load_checkpoint(&mut varmap, &weights_path) {
            raise_error!("ERR_DL_LOAD_CHECKPOINT", error = e.to_string());
        }
    } else {
        let vb = VarBuilder::from_varmap(&varmap, DType::F32, &device);
        if let Err(e) = SequenceNet::new(
            config.input_size,
            config.hidden_size,
            config.output_size,
            vb,
        ) {
            raise_error!("ERR_DL_INIT", error = e.to_string());
        }
        if let Err(e) = serialization::save_model(&varmap, &weights_path) {
            raise_error!("ERR_DL_SAVE_INITIAL", error = e.to_string());
        }
    }

    Ok((config, weights_path, varmap))
}

/// Charge un modèle via son URN, l'entraîne, et sauvegarde les nouveaux poids.
pub async fn train_model_semantic(
    manager: &CollectionsManager<'_>,
    domain: &str,
    db: &str,
    urn: &str,
    input: Vec<f32>,
    target_class: u32,
    epochs: usize,
) -> RaiseResult<f64> {
    let (config, path, varmap) = fetch_model_metadata(manager, domain, db, urn).await?;

    if input.len() != config.input_size {
        raise_error!(
            "ERR_DL_INPUT_SIZE",
            error = format!("Taille attendue: {}", config.input_size)
        );
    }

    let device = config.to_device();
    let vb = VarBuilder::from_varmap(&varmap, DType::F32, &device);

    let model = match SequenceNet::new(
        config.input_size,
        config.hidden_size,
        config.output_size,
        vb,
    ) {
        Ok(m) => m,
        Err(e) => raise_error!("ERR_DL_MODEL_INIT", error = e.to_string()),
    };

    let mut trainer = match Trainer::from_config(&varmap, &config) {
        Ok(t) => t,
        Err(e) => raise_error!("ERR_DL_TRAINER_INIT", error = e.to_string()),
    };

    let t_in = match Tensor::from_vec(input, (1usize, 1usize, config.input_size), &device) {
        Ok(t) => t,
        Err(e) => raise_error!("ERR_DL_TENSOR_IN", error = e.to_string()),
    };
    let t_tgt = match Tensor::from_vec(vec![target_class], (1usize, 1usize), &device) {
        Ok(t) => t,
        Err(e) => raise_error!("ERR_DL_TENSOR_TGT", error = e.to_string()),
    };

    let mut final_loss = 0.0;
    for _ in 0..epochs {
        final_loss = match trainer.train_step(&model, &t_in, &t_tgt) {
            Ok(l) => l,
            Err(e) => raise_error!("ERR_DL_TRAIN_STEP", error = e.to_string()),
        };
    }

    if let Err(e) = serialization::save_model(&varmap, &path) {
        raise_error!("ERR_DL_SAVE_UPDATE", error = e.to_string());
    }

    Ok(final_loss)
}

/// Charge un modèle via son URN et exécute une prédiction.
pub async fn predict_semantic(
    manager: &CollectionsManager<'_>,
    domain: &str,
    db: &str,
    urn: &str,
    input: Vec<f32>,
) -> RaiseResult<Vec<f32>> {
    let (config, path, _) = fetch_model_metadata(manager, domain, db, urn).await?;

    if input.len() != config.input_size {
        raise_error!(
            "ERR_DL_INPUT_SIZE",
            error = format!("Taille attendue: {}", config.input_size)
        );
    }

    let device = config.to_device();

    let model = match serialization::load_model(&path, &config) {
        Ok(m) => m,
        Err(e) => raise_error!("ERR_DL_LOAD", error = e.to_string()),
    };

    let t_in = match Tensor::from_vec(input, (1usize, 1usize, config.input_size), &device) {
        Ok(t) => t,
        Err(e) => raise_error!("ERR_DL_TENSOR_IN", error = e.to_string()),
    };

    let output = match model.forward(&t_in) {
        Ok(t) => t,
        Err(e) => raise_error!("ERR_DL_FORWARD", error = e.to_string()),
    };

    match output.flatten_all().and_then(|t| t.to_vec1::<f32>()) {
        Ok(v) => Ok(v),
        Err(e) => raise_error!("ERR_DL_EXTRACT", error = e.to_string()),
    }
}

// =========================================================================
// TESTS UNITAIRES ET D'INTÉGRATION DE L'API SÉMANTIQUE
// =========================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::data::json::JsonValue;
    use crate::utils::prelude::{async_test, fs};
    use crate::utils::testing::{mock, AgentDbSandbox};

    /// Test complet du workflow : Injection DB -> Résolution dynamique -> Entraînement -> Prédiction
    #[async_test]
    async fn test_api_semantic_full_workflow() -> RaiseResult<()> {
        mock::inject_mock_config().await; // 🎯 FIX : Synchronisation AppConfig / Sandbox
        let sandbox = AgentDbSandbox::new().await;
        let model_id = "4c0e7064-d672-44fe-9cc8-413ff2f841ec";

        let model_doc: JsonValue = json_value!({
            "_id": model_id,
            "handle": "test_routing_v1",
            "hyperparameters": { "input_size": 5, "hidden_size": 10, "output_size": 3, "learning_rate": 0.001 }
        });

        let manager = CollectionsManager::new(
            &sandbox.db,
            &sandbox.config.system_domain,
            &sandbox.config.system_db,
        );

        sandbox
            .db
            .write_document(
                &sandbox.config.system_domain,
                &sandbox.config.system_db,
                "dl_models",
                model_id,
                &model_doc,
            )
            .await
            .unwrap();

        let urn = "ref:dl_models:handle:test_routing_v1";
        let input = vec![0.5f32; 5];

        let loss = train_model_semantic(
            &manager,
            &sandbox.config.system_domain,
            &sandbox.config.system_db,
            urn,
            input.clone(),
            1,
            2,
        )
        .await?;
        assert!(loss >= 0.0, "La loss doit être positive");

        let prediction = predict_semantic(
            &manager,
            &sandbox.config.system_domain,
            &sandbox.config.system_db,
            urn,
            input,
        )
        .await?;
        assert_eq!(
            prediction.len(),
            3,
            "La sortie doit correspondre au output_size (3)"
        );

        Ok(())
    }

    /// Teste que le chemin physique généré dynamiquement correspond bien à la convention déterministe
    #[async_test]
    async fn test_api_semantic_deterministic_path() -> RaiseResult<()> {
        mock::inject_mock_config().await; // 🎯 FIX : Synchronisation AppConfig / Sandbox
        let sandbox = AgentDbSandbox::new().await;
        let model_id = "path-test-9999-uuid";

        let model_doc: JsonValue = json_value!({
            "_id": model_id,
            "handle": "test_path_resolution",
            "hyperparameters": { "input_size": 2, "hidden_size": 4, "output_size": 1 }
        });

        let manager = CollectionsManager::new(
            &sandbox.db,
            &sandbox.config.system_domain,
            &sandbox.config.system_db,
        );

        sandbox
            .db
            .write_document(
                &sandbox.config.system_domain,
                &sandbox.config.system_db,
                "dl_models",
                model_id,
                &model_doc,
            )
            .await
            .unwrap();

        let urn = "ref:dl_models:_id:path-test-9999-uuid";

        // 🎯 FIX : On utilise le ? pour que le test explose ici si le modèle refuse de se créer
        predict_semantic(
            &manager,
            &sandbox.config.system_domain,
            &sandbox.config.system_db,
            urn,
            vec![0.5f32; 2],
        )
        .await?;

        // 🎯 FIX : On s'aligne sur l'AppConfig qui est la véritable source de vérité du moteur
        let domain_root = AppConfig::get().get_path("PATH_RAISE_DOMAIN").unwrap();
        let expected_path = domain_root
            .join(&sandbox.config.system_domain)
            .join(&sandbox.config.system_db)
            .join("tensors")
            .join("dl_models")
            .join(format!("{}.safetensors", model_id));

        assert!(
            fs::exists_async(&expected_path).await,
            "Le fichier .safetensors n'a pas été créé au chemin déterministe attendu : {:?}",
            expected_path
        );

        Ok(())
    }

    /// Teste le comportement face à une taille d'entrée qui ne respecte pas le JSON
    #[async_test]
    async fn test_api_semantic_invalid_input_size() -> RaiseResult<()> {
        mock::inject_mock_config().await;
        let sandbox = AgentDbSandbox::new().await;

        let model_doc: JsonValue = json_value!({
            "_id": "invalid_size_test",
            "handle": "test_invalid_size",
            "hyperparameters": { "input_size": 5, "hidden_size": 10, "output_size": 3 }
        });

        let manager = CollectionsManager::new(
            &sandbox.db,
            &sandbox.config.system_domain,
            &sandbox.config.system_db,
        );

        sandbox
            .db
            .write_document(
                &sandbox.config.system_domain,
                &sandbox.config.system_db,
                "dl_models",
                "invalid_size_test",
                &model_doc,
            )
            .await
            .unwrap();

        let urn = "ref:dl_models:handle:test_invalid_size";

        // Création d'un input volontairement erroné (6 au lieu de 5)
        let bad_input = vec![0.5f32; 6];

        let res = train_model_semantic(
            &manager,
            &sandbox.config.system_domain,
            &sandbox.config.system_db,
            urn,
            bad_input,
            1,
            1,
        )
        .await;

        assert!(
            res.is_err(),
            "L'API aurait dû rejeter l'entraînement avec une dimension incorrecte"
        );

        Ok(())
    }
}
