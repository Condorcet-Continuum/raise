// FICHIER : src-tauri/src/commands/ai_commands.rs

use crate::ai::agents::AgentResult;
use crate::ai::orchestrator::AiOrchestrator;
use crate::utils::prelude::*;

// Import Moteur Natif
use crate::ai::llm::NativeLlmState;

// Imports Deep Learning
use crate::ai::deep_learning::{
    models::sequence_net::SequenceNet, serialization, trainer::Trainer,
};
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

use tauri::{command, State};

// --- STATES ---
pub struct AiState(pub AsyncMutex<Option<SharedRef<AsyncMutex<AiOrchestrator>>>>);

impl AiState {
    pub fn new(orch: Option<SharedRef<AsyncMutex<AiOrchestrator>>>) -> Self {
        Self(AsyncMutex::new(orch))
    }
}

pub struct DlState {
    pub model: SyncMutex<Option<SequenceNet>>,
    pub varmap: SyncMutex<Option<VarMap>>,
}

impl DlState {
    pub fn new() -> Self {
        Self {
            model: SyncMutex::new(None),
            varmap: SyncMutex::new(None),
        }
    }
}

impl Default for DlState {
    fn default() -> Self {
        Self::new()
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
                    "hint": "Échec du nettoyage de l'historique. Vérifiez si l'orchestrateur est dans un état verrouillé ou si la connexion au modèle est rompue."
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
                    "content_len": content.len(),
                    "hint": "Échec de l'indexation. Vérifiez le format du document ou la base vectorielle."
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
            error = "SHARED_ORCHESTRATOR_UNSET",
            context = json_value!({
                "action": "learn_document_request",
                "hint": "L'orchestrateur est absent du Guard. L'IA doit être initialisée avant l'apprentissage."
            })
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
            error = "ORCHESTRATOR_UNINITIALIZED",
            context = json_value!({
                "action": "confirm_learning",
                "hint": "L'orchestrateur IA doit être initialisé avant de confirmer un apprentissage."
            })
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
                context = json_value!({
                    "received_value": unknown,
                    "allowed_values": ["Create", "Delete"]
                })
            );
        }
    };

    let props = UnorderedMap::new();
    let state_before = ArcadiaElement {
        id: "root".to_string(),
        name: NameType::String("Context".to_string()),
        kind: "Context".to_string(),
        description: None,
        properties: props.clone(),
    };
    let state_after = ArcadiaElement {
        id: "new".to_string(),
        name: NameType::String(entity_name),
        kind: entity_kind,
        description: Some("Feedback".to_string()),
        properties: props,
    };

    match orchestrator
        .reinforce_learning(&state_before, intent, &state_after)
        .await
    {
        Ok(loss) => Ok(format!("Renforcement terminé. Loss: {:.5}", loss)),
        Err(e) => raise_error!(
            "ERR_AI_REINFORCEMENT_FAILED",
            error = e,
            context = json_value!({
                "action": "reinforce_learning",
                "intent": action_intent,
                "hint": "L'ajustement des poids a échoué. Vérifiez la structure des tenseurs de feedback."
            })
        ),
    }
}

#[command]
pub async fn ai_chat(ai_state: State<'_, AiState>, user_input: String) -> RaiseResult<AgentResult> {
    let guard = ai_state.0.lock().await;

    if let Some(shared_orch) = &*guard {
        let mut orchestrator = shared_orch.lock().await;

        match orchestrator.execute_workflow(&user_input).await {
            Ok(res) => Ok(res),
            Err(e) => raise_error!(
                "ERR_AI_WORKFLOW_EXECUTION",
                error = e,
                context = json_value!({
                    "action": "orchestrate_workflow",
                    "input_preview": user_input.chars().take(50).collect::<String>(),
                    "hint": "Le workflow a échoué. Vérifiez la connectivité aux modèles ou la logique des prompts."
                })
            ),
        }
    } else {
        raise_error!(
            "ERR_AI_SYSTEM_NOT_READY",
            error = "ORCHESTRATOR_UNINITIALIZED",
            context = json_value!({
                "action": "delegate_to_workflow",
                "hint": "L'orchestrateur partagé est vide. L'initialisation a probablement échoué au démarrage."
            })
        );
    }
}

// --- COMMANDES LEGACY & DL (Conservées) ---

#[command]
pub async fn ask_native_llm(
    state: State<'_, NativeLlmState>,
    sys: String,
    usr: String,
) -> RaiseResult<String> {
    let mut guard = match state.0.lock() {
        Ok(lock_guard) => lock_guard,
        Err(_) => raise_error!(
            "ERR_SYS_MUTEX_POISONED",
            context = json_value!({
                "component": "AiState",
                "action": "access_shared_state"
            })
        ),
    };
    if let Some(engine) = guard.as_mut() {
        match engine.generate(&sys, &usr, 1000) {
            Ok(output) => Ok(output),
            Err(e) => raise_error!(
                "ERR_AI_GENERATION_FAILED",
                error = e,
                context = json_value!({
                    "max_tokens": 1000,
                    "sys_prompt_len": sys.len(),
                    "usr_prompt_len": usr.len()
                })
            ),
        }
    } else {
        raise_error!(
            "ERR_AI_ENGINE_NOT_LOADED",
            error = "MODEL_GUARD_EMPTY",
            context = json_value!({
                "action": "access_generation_engine"
            })
        );
    }
}

#[command]
pub fn init_dl_model(state: State<'_, DlState>) -> RaiseResult<String> {
    let config = &AppConfig::get().deep_learning;
    let device = config.to_device();

    let varmap = VarMap::new();
    let vb = VarBuilder::from_varmap(&varmap, DType::F32, &device);

    let model = match SequenceNet::new(
        config.input_size,
        config.hidden_size,
        config.output_size,
        vb,
    ) {
        Ok(m) => m,
        Err(e) => raise_error!(
            "ERR_AI_MODEL_INIT_FAIL",
            error = e,
            context = json_value!({
                "input_size": config.input_size,
                "hidden_size": config.hidden_size,
                "output_size": config.output_size
            })
        ),
    };

    *state.model.lock().unwrap() = Some(model);
    *state.varmap.lock().unwrap() = Some(varmap);
    Ok("OK".to_string())
}

#[command]
pub fn run_dl_prediction(state: State<'_, DlState>, input: Vec<f32>) -> RaiseResult<Vec<f32>> {
    let config = &AppConfig::get().deep_learning;
    let device = config.to_device();

    let guard = state.model.lock().unwrap();
    if let Some(model) = &*guard {
        let input_len = input.len();
        let t = match Tensor::from_vec(input, (1, 1, input_len), &device) {
            Ok(tensor) => tensor,
            Err(e) => raise_error!(
                "ERR_MODEL_INPUT_TENSOR",
                error = e,
                context = json_value!({ "expected_shape": [1, 1, input_len] })
            ),
        };

        let out = match model.forward(&t) {
            Ok(output) => output,
            Err(e) => raise_error!("ERR_MODEL_FORWARD_PASS", error = e),
        };

        match out.flatten_all().and_then(|o| o.to_vec1::<f32>()) {
            Ok(vec) => Ok(vec),
            Err(e) => raise_error!("ERR_MODEL_OUTPUT_CONVERSION", error = e),
        }
    } else {
        raise_error!("ERR_MODEL_NOT_LOADED", error = "MODEL_GUARD_IS_NONE");
    }
}

#[command]
pub fn train_dl_step(state: State<'_, DlState>, input: Vec<f32>, target: u32) -> RaiseResult<f64> {
    let config = &AppConfig::get().deep_learning;
    let device = config.to_device();

    let mg = state.model.lock().unwrap();
    let vg = state.varmap.lock().unwrap();

    if let (Some(model), Some(vars)) = (&*mg, &*vg) {
        let input_len = input.len();

        let t_in = match Tensor::from_vec(input, (1, 1, input_len), &device) {
            Ok(t) => t,
            Err(e) => raise_error!("ERR_TRAIN_INPUT_TENSOR", error = e),
        };

        let t_tgt = match Tensor::from_vec(vec![target], (1, 1), &device) {
            Ok(t) => t,
            Err(e) => raise_error!("ERR_TRAIN_TARGET_TENSOR", error = e),
        };

        match Trainer::from_config(vars, config).train_step(model, &t_in, &t_tgt) {
            Ok(loss) => Ok(loss),
            Err(e) => raise_error!("ERR_TRAIN_STEP_FAILURE", error = e),
        }
    } else {
        raise_error!(
            "ERR_TRAIN_COMPONENTS_MISSING",
            error = "MODEL_OR_VARS_UNSET"
        );
    }
}

#[command]
pub fn save_dl_model(state: State<'_, DlState>, path: String) -> RaiseResult<String> {
    let vg = state.varmap.lock().unwrap();
    if let Some(vars) = &*vg {
        let path_buf = PathBuf::from(path);
        let path_display = path_buf.to_string_lossy().to_string();

        if let Err(e) = serialization::save_model(vars, path_buf) {
            raise_error!(
                "ERR_MODEL_SAVE_FAILURE",
                error = e,
                context = json_value!({"path": path_display})
            );
        }

        Ok(format!("Model successfully saved to {}", path_display))
    } else {
        raise_error!("ERR_MODEL_SAVE_EMPTY", error = "NO_VARIABLES_IN_GUARD");
    }
}

#[command]
pub fn load_dl_model(state: State<'_, DlState>, path: String) -> RaiseResult<String> {
    let config = &AppConfig::get().deep_learning;

    let m = match serialization::load_model(PathBuf::from(path.clone()), config) {
        Ok(model) => model,
        Err(e) => raise_error!(
            "ERR_DL_MODEL_LOAD_FAIL",
            error = e,
            context = json_value!({"path": path})
        ),
    };

    let mut model_guard = match state.model.lock() {
        Ok(guard) => guard,
        Err(_) => raise_error!(
            "ERR_SYS_MUTEX_POISONED",
            context = json_value!({"component": "DlState.model"})
        ),
    };

    let mut varmap_guard = match state.varmap.lock() {
        Ok(guard) => guard,
        Err(_) => raise_error!(
            "ERR_SYS_MUTEX_POISONED",
            context = json_value!({"component": "DlState.varmap"})
        ),
    };

    *model_guard = Some(m);
    *varmap_guard = None;

    Ok("Loaded".to_string())
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

    // 1. Initialisation du Manager (Porte d'entrée officielle de la DB)
    let db_config = JsonDbConfig::new(path_buf.clone());
    let storage = StorageEngine::new(db_config);
    let manager = CollectionsManager::new(&storage, &config.system_domain, &config.system_db);

    // 2. Extraction de la Topologie Arcadia (Matrice A) via le Manager
    let adjacency = GraphAdjacency::build_from_store(&manager, &device).await?;

    // 3. Extraction de la Sémantique NLP (Matrice H) via le Manager
    let mut engine = EmbeddingEngine::new(&manager).await?;
    let features =
        GraphFeatures::build_from_store(&adjacency.index_to_uri, &mut engine, &device).await?;

    // 🎯 FIX : Construction des Arêtes (Sparse Format COO) depuis la matrice extraite
    let n = adjacency.index_to_uri.len();
    let adj_data = adjacency
        .matrix
        .flatten_all()
        .map_err(|e| build_error!("ERR_GNN_MATRIX_FLATTEN", error = e.to_string()))?
        .to_vec1::<f32>()
        .map_err(|e| build_error!("ERR_GNN_VEC_CONVERSION", error = e.to_string()))?;

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

    let edge_src = Tensor::new(src_indices, &device)
        .map_err(|e| build_error!("ERR_GNN_TENSOR_SRC", error = e.to_string()))?;
    let edge_dst = Tensor::new(dst_indices, &device)
        .map_err(|e| build_error!("ERR_GNN_TENSOR_DST", error = e.to_string()))?;

    // 4. Initialisation du Modèle GNN
    let varmap = VarMap::new();
    let vb = VarBuilder::from_varmap(&varmap, DType::F32, &device);
    let in_dim = features.matrix.dims()[1];
    let model = ArcadiaGnnModel::new(in_dim, in_dim / 2, 32, vb).await?;

    // 5. L'Expérience : Mesure du rapprochement sémantique
    let sim_initial = model
        .compute_similarity(&features.matrix, &adjacency, &uri_a, &uri_b)
        .await?;

    // 🎯 FIX : Passage des bons arguments (les tenseurs creux et les features) au modèle
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
