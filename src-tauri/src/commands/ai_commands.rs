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
pub async fn ai_reset(ai_state: State<'_, AiState>) -> Result<()> {
    let guard = ai_state.0.lock().await;
    if let Some(shared_orch) = &*guard {
        let mut orchestrator = shared_orch.lock().await;
        orchestrator
            .clear_history()
            .await
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[command]
pub async fn ai_learn_text(
    ai_state: State<'_, AiState>,
    content: String,
    source: String,
) -> Result<String> {
    let guard = ai_state.0.lock().await;
    if let Some(shared_orch) = &*guard {
        let mut orchestrator = shared_orch.lock().await;
        let chunks_count = orchestrator
            .learn_document(&content, &source)
            .await
            .map_err(|e| e.to_string())?;
        Ok(format!("Document appris ({} fragments).", chunks_count))
    } else {
        Err(AppError::from("IA non initialisée.".to_string()))
    }
}

#[command]
pub async fn ai_confirm_learning(
    ai_state: State<'_, AiState>,
    action_intent: String,
    entity_name: String,
    entity_kind: String,
) -> Result<String> {
    let guard = ai_state.0.lock().await;
    if let Some(shared_orch) = &*guard {
        let orchestrator = shared_orch.lock().await;
        let intent = match action_intent.as_str() {
            "Create" => CommandType::Create,
            "Delete" => CommandType::Delete,
            _ => return Err(AppError::from("Action inconnue".to_string())),
        };

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

        let loss = orchestrator
            .reinforce_learning(&state_before, intent, &state_after)
            .await
            .map_err(|e| e.to_string())?;
        Ok(format!("Loss: {:.5}", loss))
    } else {
        Err(AppError::from("IA non initialisée."))
    }
}

/// Point d'entrée principal du Chat IA.
/// Désormais, cette commande délègue TOUT à l'Orchestrateur unifié.
/// L'orchestrateur gère lui-même : RAG, Intention, Agents, Boucle ACL, Storage.
#[command]
pub async fn ai_chat(ai_state: State<'_, AiState>, user_input: String) -> Result<AgentResult> {
    // 1. Récupération de l'Orchestrateur partagé
    let guard = ai_state.0.lock().await;

    if let Some(shared_orch) = &*guard {
        let mut orchestrator = shared_orch.lock().await;

        // 2. Délégation complète (Le cerveau fait tout)
        orchestrator
            .execute_workflow(&user_input)
            .await
            .map_err(|e| AppError::from(format!("Erreur Workflow: {}", e)))
    } else {
        Err(AppError::from(
            "Système IA non initialisé (vérifiez les logs serveur).",
        ))
    }
}

// --- COMMANDES LEGACY & DL (Conservées) ---

#[command]
pub async fn ask_native_llm(
    state: State<'_, NativeLlmState>,
    sys: String,
    usr: String,
) -> Result<String> {
    let mut guard = state.0.lock().map_err(|_| "Lock error".to_string())?;
    if let Some(engine) = guard.as_mut() {
        engine
            .generate(&sys, &usr, 1000)
            .map_err(|e| AppError::from(e.to_string()))
    } else {
        Err(AppError::from("Chargement modèle..."))
    }
}

#[command]
pub fn init_dl_model(state: State<'_, DlState>) -> Result<String> {
    // 🎯 On récupère la config globale (SSOT)
    let config = &AppConfig::get().deep_learning;
    let device = config.to_device();

    let varmap = VarMap::new();
    let vb = VarBuilder::from_varmap(&varmap, DType::F32, &device);

    // On utilise les dimensions de la config au lieu des paramètres i, h, o
    let model = SequenceNet::new(
        config.input_size,
        config.hidden_size,
        config.output_size,
        vb,
    )
    .map_err(|e| e.to_string())?;

    *state.model.lock().unwrap() = Some(model);
    *state.varmap.lock().unwrap() = Some(varmap);
    Ok("OK".to_string())
}

#[command]
pub fn run_dl_prediction(state: State<'_, DlState>, input: Vec<f32>) -> Result<Vec<f32>> {
    let config = &AppConfig::get().deep_learning;
    let device = config.to_device(); // 🎯 Utilise le périphérique config (CUDA si BlackWell)

    let guard = state.model.lock().unwrap();
    if let Some(model) = &*guard {
        let t = Tensor::from_vec(input.clone(), (1, 1, input.len()), &device)
            .map_err(|e| AppError::from(e.to_string()))?;
        let out = model
            .forward(&t)
            .map_err(|e| AppError::from(e.to_string()))?;

        out.flatten_all()
            .map_err(|e| AppError::from(e.to_string()))?
            .to_vec1::<f32>()
            .map_err(|e| AppError::from(e.to_string()))
    } else {
        Err(AppError::from("No model"))
    }
}

#[command]
pub fn train_dl_step(state: State<'_, DlState>, input: Vec<f32>, target: u32) -> Result<f64> {
    let config = &AppConfig::get().deep_learning;
    let device = config.to_device();

    let mg = state.model.lock().unwrap();
    let vg = state.varmap.lock().unwrap();

    if let (Some(m), Some(v)) = (&*mg, &*vg) {
        let t_in = Tensor::from_vec(input.clone(), (1, 1, input.len()), &device)
            .map_err(|e| AppError::from(e.to_string()))?;
        let t_tgt = Tensor::from_vec(vec![target], (1, 1), &device)
            .map_err(|e| AppError::from(e.to_string()))?;

        // 🎯 Utilisation du constructeur intelligent
        Trainer::from_config(v, config)
            .train_step(m, &t_in, &t_tgt)
            .map_err(|e| AppError::from(e.to_string()))
    } else {
        Err(AppError::from("No model"))
    }
}

#[command]
pub fn save_dl_model(state: State<'_, DlState>, path: String) -> Result<String> {
    let vg = state.varmap.lock().unwrap();
    if let Some(v) = &*vg {
        serialization::save_model(v, PathBuf::from(path)).map_err(|e| e.to_string())?;
        Ok("Saved".to_string())
    } else {
        Err(AppError::from("No model"))
    }
}

#[command]
pub fn load_dl_model(state: State<'_, DlState>, path: String) -> Result<String> {
    let config = &AppConfig::get().deep_learning;

    let m = serialization::load_model(PathBuf::from(path), config).map_err(|e| e.to_string())?;

    *state.model.lock().unwrap() = Some(m);
    *state.varmap.lock().unwrap() = None;
    Ok("Loaded".to_string())
}
