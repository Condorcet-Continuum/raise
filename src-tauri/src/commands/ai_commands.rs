// FICHIER : src-tauri/src/commands/ai_commands.rs

use crate::utils::config::AppConfig;
use crate::utils::{data::HashMap, io::PathBuf, prelude::*, Arc};
use std::sync::Mutex as SyncMutex;
use tokio::sync::Mutex as AsyncMutex;

use crate::ai::agents::AgentResult;
use crate::ai::orchestrator::AiOrchestrator;

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

use tauri::{command, State};

// --- STATES ---
pub struct AiState(pub AsyncMutex<Option<Arc<AsyncMutex<AiOrchestrator>>>>);

impl AiState {
    pub fn new(orch: Option<Arc<AsyncMutex<AiOrchestrator>>>) -> Self {
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

        // On remplace le map_err par un match explicite
        if let Err(e) = orchestrator.clear_history().await {
            raise_error!(
                "ERR_AI_HISTORY_CLEAR_FAIL",
                error = e,
                context = json!({
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

        // 1. Apprentissage avec capture de contexte (Source + Taille)
        let chunks_count = match orchestrator.learn_document(&content, &source).await {
            Ok(count) => count,
            Err(e) => raise_error!(
                "ERR_AI_LEARN_DOCUMENT_FAILURE",
                error = e,
                context = json!({
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
        // 2. Erreur d'état système
        raise_error!(
            "ERR_AI_ORCHESTRATOR_NOT_READY",
            error = "SHARED_ORCHESTRATOR_UNSET",
            context = json!({
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

    // 1. Vérification de l'état de l'Orchestrateur
    let Some(shared_orch) = &*guard else {
        raise_error!(
            "ERR_AI_SYSTEM_NOT_READY",
            error = "ORCHESTRATOR_UNINITIALIZED",
            context = json!({
                "action": "confirm_learning",
                "hint": "L'orchestrateur IA doit être initialisé avant de confirmer un apprentissage."
            })
        );
    };

    let orchestrator = shared_orch.lock().await;

    // 2. Mapping de l'intention (Correction du unreachable)
    let intent = match action_intent.as_str() {
        "Create" => CommandType::Create,
        "Delete" => CommandType::Delete,
        unknown => {
            raise_error!(
                "ERR_CLI_UNKNOWN_ACTION",
                error = "INVALID_COMMAND_TYPE",
                context = json!({
                    "received_value": unknown,
                    "allowed_values": ["Create", "Delete"]
                })
            );
        }
    };

    // 3. Construction des états (Simplifiée)
    let props = HashMap::new();
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

    // 4. Exécution du renforcement avec capture de Loss
    match orchestrator
        .reinforce_learning(&state_before, intent, &state_after)
        .await
    {
        Ok(loss) => Ok(format!("Renforcement terminé. Loss: {:.5}", loss)),
        Err(e) => raise_error!(
            "ERR_AI_REINFORCEMENT_FAILED",
            error = e,
            context = json!({
                "action": "reinforce_learning",
                "intent": action_intent,
                "hint": "L'ajustement des poids a échoué. Vérifiez la structure des tenseurs de feedback."
            })
        ),
    }
}

/// Point d'entrée principal du Chat IA.
/// Désormais, cette commande délègue TOUT à l'Orchestrateur unifié.
/// L'orchestrateur gère lui-même : RAG, Intention, Agents, Boucle ACL, Storage.
#[command]
pub async fn ai_chat(ai_state: State<'_, AiState>, user_input: String) -> RaiseResult<AgentResult> {
    // 1. Récupération de l'Orchestrateur partagé
    let guard = ai_state.0.lock().await;

    if let Some(shared_orch) = &*guard {
        let mut orchestrator = shared_orch.lock().await;

        // 1. Exécution du workflow avec capture d'erreur riche
        match orchestrator.execute_workflow(&user_input).await {
            Ok(res) => Ok(res),
            Err(e) => raise_error!(
                "ERR_AI_WORKFLOW_EXECUTION",
                error = e,
                context = json!({
                    "action": "orchestrate_workflow",
                    "input_preview": user_input.chars().take(50).collect::<String>(),
                    "hint": "Le workflow a échoué. Vérifiez la connectivité aux modèles ou la logique des prompts."
                })
            ),
        }
    } else {
        // 2. Erreur d'état système non initialisé
        raise_error!(
            "ERR_AI_SYSTEM_NOT_READY",
            error = "ORCHESTRATOR_UNINITIALIZED",
            context = json!({
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
            context = json!({
                "component": "AiState",
                "action": "access_shared_state",
                "hint": "Le Mutex est empoisonné suite à une panique. L'état partagé est corrompu."
            })
        ),
    };
    if let Some(engine) = guard.as_mut() {
        // 1. Exécution de la génération avec contexte riche
        match engine.generate(&sys, &usr, 1000) {
            Ok(output) => Ok(output),
            Err(e) => raise_error!(
                "ERR_AI_GENERATION_FAILED",
                error = e,
                context = json!({
                    "action": "model_inference",
                    "max_tokens": 1000,
                    "sys_prompt_len": sys.len(),
                    "usr_prompt_len": usr.len(),
                    "hint": "Échec de la génération. Vérifiez la mémoire GPU ou la validité des prompts."
                })
            ),
        }
    } else {
        // 2. Erreur d'état : Moteur non prêt
        raise_error!(
            "ERR_AI_ENGINE_NOT_LOADED",
            error = "MODEL_GUARD_EMPTY",
            context = json!({
                "action": "access_generation_engine",
                "state": "loading_or_failed",
                "hint": "Le moteur de génération est manquant. Attendez la fin du chargement ou vérifiez les erreurs d'init."
            })
        );
    }
}

#[command]
pub fn init_dl_model(state: State<'_, DlState>) -> RaiseResult<String> {
    // 🎯 On récupère la config globale (SSOT)
    let config = &AppConfig::get().deep_learning;
    let device = config.to_device();

    let varmap = VarMap::new();
    let vb = VarBuilder::from_varmap(&varmap, DType::F32, &device);

    // On utilise les dimensions de la config au lieu des paramètres i, h, o
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
            context = json!({
                "input_size": config.input_size,
                "hidden_size": config.hidden_size,
                "output_size": config.output_size,
                "action": "initialize_sequence_net",
                "hint": "Échec de l'allocation des tenseurs. Vérifiez la compatibilité des dimensions ou la mémoire GPU/RAM disponible."
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
    let device = config.to_device(); // 🎯 Utilise le périphérique config (CUDA si BlackWell)

    let guard = state.model.lock().unwrap();
    if let Some(model) = &*guard {
        // 1. Préparation du Tenseur d'entrée
        let input_len = input.len();
        let t = match Tensor::from_vec(input, (1, 1, input_len), &device) {
            Ok(tensor) => tensor,
            Err(e) => raise_error!(
                "ERR_MODEL_INPUT_TENSOR",
                error = e,
                context = json!({
                    "action": "create_input_tensor",
                    "expected_shape": [1, 1, input_len],
                    "device": format!("{:?}", device)
                })
            ),
        };

        // 2. Passe Avant (Inférence)
        let out = match model.forward(&t) {
            Ok(output) => output,
            Err(e) => raise_error!(
                "ERR_MODEL_FORWARD_PASS",
                error = e,
                context = json!({ "action": "neural_network_forward" })
            ),
        };

        // 3. Post-traitement et conversion
        match out.flatten_all().and_then(|o| o.to_vec1::<f32>()) {
            Ok(vec) => Ok(vec),
            Err(e) => raise_error!(
                "ERR_MODEL_OUTPUT_CONVERSION",
                error = e,
                context = json!({ "action": "flatten_and_convert_to_vec" })
            ),
        }
    } else {
        // 4. Erreur d'état : Modèle absent
        raise_error!(
            "ERR_MODEL_NOT_LOADED",
            error = "MODEL_GUARD_IS_NONE",
            context = json!({ "action": "prediction_attempt", "hint": "Le modèle n'est pas encore chargé dans le Guard." })
        );
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

        // 1. Préparation des données (Input & Target)
        let t_in = match Tensor::from_vec(input, (1, 1, input_len), &device) {
            Ok(t) => t,
            Err(e) => raise_error!(
                "ERR_TRAIN_INPUT_TENSOR",
                error = e,
                context = json!({
                    "action": "create_training_input",
                    "shape": [1, 1, input_len],
                    "device": format!("{:?}", device)
                })
            ),
        };

        let t_tgt = match Tensor::from_vec(vec![target], (1, 1), &device) {
            Ok(t) => t,
            Err(e) => raise_error!(
                "ERR_TRAIN_TARGET_TENSOR",
                error = e,
                context = json!({ "action": "create_training_target", "target_val": target })
            ),
        };

        // 2. Initialisation et Step d'entraînement
        match Trainer::from_config(vars, config).train_step(model, &t_in, &t_tgt) {
            Ok(loss) => Ok(loss),
            Err(e) => raise_error!(
                "ERR_TRAIN_STEP_FAILURE",
                error = e,
                context = json!({
                    "action": "execute_train_step",
                    "learning_rate": config.learning_rate,
                    "hint": "Échec de la backpropagation ou du calcul de la loss. Vérifiez l'intégrité des gradients."
                })
            ),
        }
    } else {
        // 3. Erreur d'état : Composants manquants
        raise_error!(
            "ERR_TRAIN_COMPONENTS_MISSING",
            error = "MODEL_OR_VARS_UNSET",
            context = json!({
                "action": "start_train_step",
                "model_present": mg.is_some(),
                "vars_present": vg.is_some()
            })
        );
    }
}

#[command]
pub fn save_dl_model(state: State<'_, DlState>, path: String) -> RaiseResult<String> {
    let vg = state.varmap.lock().unwrap();
    if let Some(vars) = &*vg {
        let path_buf = PathBuf::from(path);
        let path_display = path_buf.to_string_lossy().to_string();

        // 1. Sauvegarde avec capture d'erreur I/O riche
        if let Err(e) = serialization::save_model(vars, path_buf) {
            raise_error!(
                "ERR_MODEL_SAVE_FAILURE",
                error = e,
                context = json!({
                    "action": "persist_model_to_disk",
                    "path": path_display,
                    "hint": "Vérifiez l'espace disque disponible et les permissions d'écriture sur ce dossier."
                })
            );
        }

        Ok(format!("Model successfully saved to {}", path_display))
    } else {
        // 2. Erreur d'état : Rien à sauvegarder
        raise_error!(
            "ERR_MODEL_SAVE_EMPTY",
            error = "NO_VARIABLES_IN_GUARD",
            context = json!({
                "action": "attempt_save",
                "hint": "Le Guard de variables est vide. Assurez-vous que le modèle est initialisé avant de sauvegarder."
            })
        );
    }
}

#[command]
pub fn load_dl_model(state: State<'_, DlState>, path: String) -> RaiseResult<String> {
    let config = &AppConfig::get().deep_learning;

    // 1. Chargement du modèle avec Match explicite
    let m = match serialization::load_model(PathBuf::from(path.clone()), config) {
        Ok(model) => model,
        Err(e) => raise_error!(
            "ERR_DL_MODEL_LOAD_FAIL",
            error = e,
            context = json!({
                "path": path,
                "action": "serialization_load_model",
                "hint": "Le fichier modèle est peut-être corrompu ou le format n'est pas supporté (safetensors/bin)."
            })
        ),
    };

    // 2. Accès sécurisé au Mutex du modèle
    let mut model_guard = match state.model.lock() {
        Ok(guard) => guard,
        Err(_) => raise_error!(
            "ERR_SYS_MUTEX_POISONED",
            context = json!({"component": "DlState.model"})
        ),
    };

    // 3. Accès sécurisé au Mutex du varmap
    let mut varmap_guard = match state.varmap.lock() {
        Ok(guard) => guard,
        Err(_) => raise_error!(
            "ERR_SYS_MUTEX_POISONED",
            context = json!({"component": "DlState.varmap"})
        ),
    };

    // Mise à jour de l'état
    *model_guard = Some(m);
    *varmap_guard = None;

    Ok("Loaded".to_string())
}
