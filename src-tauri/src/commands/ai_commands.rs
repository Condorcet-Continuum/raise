// FICHIER : src-tauri/src/commands/ai_commands.rs

use crate::ai::agents::AgentResult;
use crate::ai::orchestrator::AiOrchestrator;
use crate::utils::prelude::*;

// Import Moteur Natif
use crate::ai::llm::NativeLlmState;

use candle_core::{DType, Tensor};
use candle_nn::{VarBuilder, VarMap};

// Imports World Model
use crate::ai::nlp::parser::CommandType;
use crate::model_engine::types::{ArcadiaElement, NameType};

// Imports GNN Arcadia
use crate::ai::deep_learning::models::gnn_model::ArcadiaGnnModel;
use crate::ai::graph_store::{GraphAdjacency, GraphFeatures};
use crate::ai::nlp::embeddings::EmbeddingEngine;
use crate::json_db::collections::manager::CollectionsManager;
use crate::json_db::storage::{JsonDbConfig, StorageEngine};

// 🎯 IMPORT POUR L'EXPORT DE DATASET
use crate::ai::training::dataset::{extract_domain_data, TrainingExample};

use tauri::{command, State};

// --- STATES ---
pub struct AiState(pub AsyncMutex<Option<SharedRef<AsyncMutex<AiOrchestrator>>>>);

impl AiState {
    pub fn new(orch: Option<SharedRef<AsyncMutex<AiOrchestrator>>>) -> Self {
        Self(AsyncMutex::new(orch))
    }
}

// --- COMMANDES ORCHESTRATION UNIFIÉE (V2) ---

#[command]
pub async fn ai_reset(ai_state: State<'_, AiState>) -> RaiseResult<()> {
    let guard = ai_state.0.lock().await;

    if let Some(shared_orch) = &*guard {
        let mut orchestrator = shared_orch.lock().await;

        if let Err(e) = orchestrator.clear_history().await {
            raise_error!(
                "ERR_AI_HISTORY_CLEAR_FAIL",
                error = e,
                context = json_value!({
                    "action": "reset_ai_orchestrator",
                    "hint": "Échec du nettoyage de l'historique."
                })
            );
        }
    }

    Ok(())
}

#[command]
pub async fn ai_learn_text(
    ai_state: State<'_, AiState>,
    content: String,
    source: String,
) -> RaiseResult<String> {
    let guard = ai_state.0.lock().await;
    if let Some(shared_orch) = &*guard {
        let mut orchestrator = shared_orch.lock().await;

        let chunks_count = match orchestrator.learn_document(&content, &source).await {
            Ok(count) => count,
            Err(e) => raise_error!(
                "ERR_AI_LEARN_DOCUMENT_FAILURE",
                error = e,
                context = json_value!({
                    "action": "ingest_document",
                    "source": source,
                    "content_len": content.len()
                })
            ),
        };

        Ok(format!(
            "Document appris avec succès ({} fragments).",
            chunks_count
        ))
    } else {
        raise_error!(
            "ERR_AI_ORCHESTRATOR_NOT_READY",
            error = "SHARED_ORCHESTRATOR_UNSET"
        );
    }
}

#[command]
pub async fn ai_confirm_learning(
    ai_state: State<'_, AiState>,
    action_intent: String,
    entity_name: String,
    entity_kind: String,
) -> RaiseResult<String> {
    let guard = ai_state.0.lock().await;

    let Some(shared_orch) = &*guard else {
        raise_error!(
            "ERR_AI_SYSTEM_NOT_READY",
            error = "ORCHESTRATOR_UNINITIALIZED"
        );
    };

    let orchestrator = shared_orch.lock().await;

    let intent = match action_intent.as_str() {
        "Create" => CommandType::Create,
        "Delete" => CommandType::Delete,
        unknown => {
            raise_error!(
                "ERR_CLI_UNKNOWN_ACTION",
                error = "INVALID_COMMAND_TYPE",
                context = json_value!({"received": unknown})
            );
        }
    };

    // 🎯 PURE GRAPH : Préparation des propriétés dynamiques
    let props_before = UnorderedMap::new();
    let state_before = ArcadiaElement {
        id: "root".to_string(),
        name: NameType::String("Context".to_string()),
        kind: "Context".to_string(),
        // description: None, <- SUPPRIMÉ
        properties: props_before,
    };

    let mut props_after = UnorderedMap::new();
    props_after.insert("description".to_string(), json_value!("Feedback"));

    let state_after = ArcadiaElement {
        id: "new".to_string(),
        name: NameType::String(entity_name),
        kind: entity_kind,
        // description: Some("Feedback".to_string()), <- SUPPRIMÉ
        properties: props_after,
    };

    match orchestrator
        .reinforce_learning(&state_before, intent, &state_after)
        .await
    {
        Ok(loss) => Ok(format!("Renforcement terminé. Loss: {:.5}", loss)),
        Err(e) => raise_error!("ERR_AI_REINFORCEMENT_FAILED", error = e),
    }
}

#[command]
pub async fn ai_chat(ai_state: State<'_, AiState>, user_input: String) -> RaiseResult<AgentResult> {
    let guard = ai_state.0.lock().await;

    if let Some(shared_orch) = &*guard {
        let mut orchestrator = shared_orch.lock().await;

        match orchestrator.execute_workflow(&user_input).await {
            Ok(res) => Ok(res),
            Err(e) => raise_error!("ERR_AI_WORKFLOW_EXECUTION", error = e),
        }
    } else {
        raise_error!(
            "ERR_AI_SYSTEM_NOT_READY",
            error = "ORCHESTRATOR_UNINITIALIZED"
        );
    }
}

#[command]
pub async fn ask_native_llm(
    state: State<'_, NativeLlmState>,
    sys: String,
    usr: String,
) -> RaiseResult<String> {
    let mut guard = match state.0.lock() {
        Ok(lock_guard) => lock_guard,
        Err(_) => raise_error!("ERR_SYS_MUTEX_POISONED"),
    };
    if let Some(engine) = guard.as_mut() {
        match engine.generate(&sys, &usr, 1000) {
            Ok(output) => Ok(output),
            Err(e) => raise_error!("ERR_AI_GENERATION_FAILED", error = e),
        }
    } else {
        raise_error!("ERR_AI_ENGINE_NOT_LOADED", error = "MODEL_GUARD_EMPTY");
    }
}

#[command]
pub async fn validate_arcadia_gnn(
    collections_path: String,
    uri_a: String,
    uri_b: String,
) -> RaiseResult<JsonValue> {
    user_info!(
        "🚀 [GNN] Lancement de l'expérience MBSE...",
        json_value!({ "a": uri_a, "b": uri_b })
    );

    let path_buf = PathBuf::from(&collections_path);
    let config = AppConfig::get();
    let device = config.deep_learning.to_device();

    let db_config = JsonDbConfig::new(path_buf.clone());
    let storage = StorageEngine::new(db_config);
    let manager = CollectionsManager::new(&storage, &config.system_domain, &config.system_db);

    let adjacency = GraphAdjacency::build_from_store(&manager, &device).await?;
    let mut engine = EmbeddingEngine::new(&manager).await?;

    let features =
        GraphFeatures::build_from_store(&manager, &adjacency.index_to_uri, &mut engine, &device)
            .await?;

    let n = adjacency.index_to_uri.len();
    let flattened = match adjacency.matrix.flatten_all() {
        Ok(matrix) => matrix,
        Err(e) => raise_error!("ERR_GNN_MATRIX_FLATTEN", error = e.to_string()),
    };

    let adj_data = match flattened.to_vec1::<f32>() {
        Ok(data) => data,
        Err(e) => raise_error!("ERR_GNN_VEC_CONVERSION", error = e.to_string()),
    };

    let mut src_indices = Vec::new();
    let mut dst_indices = Vec::new();
    for i in 0..n {
        for j in 0..n {
            if adj_data[i * n + j] > 0.5 {
                src_indices.push(i as u32);
                dst_indices.push(j as u32);
            }
        }
    }

    let edge_src = match Tensor::new(src_indices, &device) {
        Ok(tensor) => tensor,
        Err(e) => raise_error!("ERR_GNN_TENSOR_SRC", error = e.to_string()),
    };

    let edge_dst = match Tensor::new(dst_indices, &device) {
        Ok(tensor) => tensor,
        Err(e) => raise_error!("ERR_GNN_TENSOR_DST", error = e.to_string()),
    };

    let varmap = VarMap::new();
    let vb = VarBuilder::from_varmap(&varmap, DType::F32, &device);
    let in_dim = features.matrix.dims()[1];
    let model = ArcadiaGnnModel::new(in_dim, in_dim / 2, 32, vb).await?;

    let sim_initial = model
        .compute_similarity(&features.matrix, &adjacency, &uri_a, &uri_b)
        .await?;
    let final_embeddings = model
        .forward(&edge_src, &edge_dst, &features.matrix)
        .await?;
    let sim_final = model
        .compute_similarity(&final_embeddings, &adjacency, &uri_a, &uri_b)
        .await?;

    let delta = sim_final - sim_initial;
    let confirmed = delta > 0.0;

    if confirmed {
        user_success!(
            "✅ [GNN] Hypothèse confirmée : rapprochement de {:.2}%",
            json_value!(delta * 100.0)
        );
    }

    Ok(json_value!({
        "status": "success",
        "uri_a": uri_a,
        "uri_b": uri_b,
        "metrics": {
            "nlp_similarity": sim_initial,
            "gnn_similarity": sim_final,
            "improvement": delta
        },
        "hypothesis_confirmed": confirmed
    }))
}

// 🎯 DÉPLACEMENT STRATÉGIQUE : La commande Tauri pour l'export Dataset
#[command]
pub async fn ai_export_dataset(
    storage: tauri::State<'_, StorageEngine>,
    space: String,
    db_name: String,
    domain: String,
) -> RaiseResult<Vec<TrainingExample>> {
    let manager = CollectionsManager::new(storage.inner(), &space, &db_name);
    extract_domain_data(&manager, &domain).await
}

// =========================================================================
// TESTS UNITAIRES
// =========================================================================
#[cfg(test)]
mod tests_gnn_cmd {
    use super::*;
    use crate::utils::testing::AgentDbSandbox;

    #[async_test]
    async fn test_validate_arcadia_gnn_not_found_fails() {
        let sandbox = AgentDbSandbox::new().await;

        let result = validate_arcadia_gnn(
            sandbox.domain_root.to_string_lossy().to_string(),
            "la:InconnuA".to_string(),
            "la:InconnuB".to_string(),
        )
        .await;

        assert!(result.is_err());
    }
}
