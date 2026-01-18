// FICHIER : src-tauri/src/commands/ai_commands.rs

use crate::ai::agents::intent_classifier::{EngineeringIntent, IntentClassifier};
use crate::ai::agents::{
    business_agent::BusinessAgent, data_agent::DataAgent, epbs_agent::EpbsAgent,
    hardware_agent::HardwareAgent, software_agent::SoftwareAgent, system_agent::SystemAgent,
    transverse_agent::TransverseAgent, Agent, AgentContext, AgentResult,
};

// Imports pour l'Orchestrateur
use crate::ai::llm::client::LlmClient;
use crate::ai::orchestrator::AiOrchestrator;
use crate::json_db::storage::StorageEngine;
use tokio::sync::Mutex as AsyncMutex;

// Import pour le Moteur Natif
use crate::ai::llm::NativeLlmState;

// --- IMPORTS DEEP LEARNING ---
use crate::ai::deep_learning::{
    models::sequence_net::SequenceNet, serialization, trainer::Trainer,
};
use candle_core::{DType, Device, Tensor};
use candle_nn::{VarBuilder, VarMap};
use std::sync::Mutex as SyncMutex;

// --- IMPORTS WORLD MODEL ---
use crate::ai::nlp::parser::CommandType;
use crate::model_engine::types::{ArcadiaElement, NameType};
use std::collections::HashMap;
// -------------------------

use std::env;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{command, State};

// --- 1. ÉTAT GLOBAL EXISTANT (INTACT) ---
pub struct AiState(pub AsyncMutex<Option<Arc<AsyncMutex<AiOrchestrator>>>>);

impl AiState {
    pub fn new(orch: Option<Arc<AsyncMutex<AiOrchestrator>>>) -> Self {
        Self(AsyncMutex::new(orch))
    }
}

// --- 2. NOUVEL ÉTAT POUR LE DEEP LEARNING (SÉPARÉ) ---
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

// --- COMMANDES EXISTANTES ---

#[command]
pub async fn ai_reset(ai_state: State<'_, AiState>) -> Result<(), String> {
    let guard = ai_state.0.lock().await;

    if let Some(shared_orch) = &*guard {
        let mut orchestrator = shared_orch.lock().await;
        orchestrator.clear_history().map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[command]
pub async fn ai_learn_text(
    ai_state: State<'_, AiState>,
    content: String,
    source: String,
) -> Result<String, String> {
    let guard = ai_state.0.lock().await;

    if let Some(shared_orch) = &*guard {
        let mut orchestrator = shared_orch.lock().await;
        let chunks_count = orchestrator
            .learn_document(&content, &source)
            .await
            .map_err(|e| e.to_string())?;

        Ok(format!(
            "Document appris avec succès ({} fragments).",
            chunks_count
        ))
    } else {
        Err("L'IA n'est pas initialisée.".to_string())
    }
}

// ------------------------------------------------
// --- AJOUT COMMANDE FEEDBACK ---
#[command]
pub async fn ai_confirm_learning(
    ai_state: State<'_, AiState>,
    action_intent: String,
    entity_name: String,
    entity_kind: String,
) -> Result<String, String> {
    let guard = ai_state.0.lock().await;

    if let Some(shared_orch) = &*guard {
        let orchestrator = shared_orch.lock().await;

        let intent = match action_intent.as_str() {
            "Create" => CommandType::Create,
            "Delete" => CommandType::Delete,
            _ => CommandType::Unknown,
        };

        if intent == CommandType::Unknown {
            return Err("Type d'action inconnu (Create/Delete attendu)".to_string());
        }

        let props = HashMap::new();
        let state_before = ArcadiaElement {
            id: "root_context".to_string(),
            name: NameType::String("SystemContext".to_string()),
            kind: "https://arcadia/core#Context".to_string(),
            description: None,
            properties: props.clone(),
        };

        let state_after = ArcadiaElement {
            id: "new_element".to_string(),
            name: NameType::String(entity_name),
            kind: entity_kind,
            description: Some("Element created by user feedback".to_string()),
            properties: props,
        };

        let loss = orchestrator
            .reinforce_learning(&state_before, intent, &state_after)
            .await
            .map_err(|e| e.to_string())?;

        Ok(format!("Cerveau mis à jour. Loss: {:.5}", loss))
    } else {
        Err("L'IA n'est pas initialisée.".to_string())
    }
}
// -----------------------------

#[command]
pub async fn ai_chat(
    storage: State<'_, StorageEngine>,
    ai_state: State<'_, AiState>,
    user_input: String,
) -> Result<AgentResult, String> {
    let _mode_dual = env::var("RAISE_MODE_DUAL").unwrap_or_else(|_| "false".to_string()) == "true";
    let gemini_key = env::var("RAISE_GEMINI_KEY").unwrap_or_default();
    let model_name = env::var("RAISE_MODEL_NAME").ok();

    let local_url_raw =
        env::var("RAISE_LOCAL_URL").unwrap_or_else(|_| "http://127.0.0.1:8081".to_string());
    let local_url = local_url_raw.replace("localhost", "127.0.0.1");

    let domain_path = env::var("PATH_RAISE_DOMAIN")
        .map(PathBuf::from)
        .unwrap_or_else(|_| std::env::current_dir().unwrap().join("data"));
    let dataset_path = env::var("PATH_RAISE_DATASET")
        .map(PathBuf::from)
        .unwrap_or_else(|_| std::env::current_dir().unwrap().join("dataset"));

    let client = LlmClient::new(&local_url, &gemini_key, model_name.clone());

    let classifier = IntentClassifier::new(client.clone());
    let intent = classifier.classify(&user_input).await;

    let ctx = AgentContext::new(
        Arc::new(storage.inner().clone()),
        client.clone(),
        domain_path,
        dataset_path,
    );

    let result = match intent {
        EngineeringIntent::DefineBusinessUseCase { .. } => {
            BusinessAgent::new().process(&ctx, &intent).await
        }
        EngineeringIntent::CreateElement { ref layer, .. } if layer == "SA" => {
            SystemAgent::new().process(&ctx, &intent).await
        }
        EngineeringIntent::CreateElement {
            ref layer,
            ref element_type,
            ..
        } if layer == "LA" || element_type.to_lowercase().contains("software") => {
            SoftwareAgent::new().process(&ctx, &intent).await
        }
        EngineeringIntent::CreateElement { ref layer, .. } if layer == "PA" => {
            HardwareAgent::new().process(&ctx, &intent).await
        }
        EngineeringIntent::CreateElement { ref layer, .. } if layer == "EPBS" => {
            EpbsAgent::new().process(&ctx, &intent).await
        }
        EngineeringIntent::CreateElement { ref layer, .. } if layer == "DATA" => {
            DataAgent::new().process(&ctx, &intent).await
        }
        EngineeringIntent::CreateElement { ref layer, .. } if layer == "TRANSVERSE" => {
            TransverseAgent::new().process(&ctx, &intent).await
        }
        EngineeringIntent::GenerateCode { .. } => SoftwareAgent::new().process(&ctx, &intent).await,

        EngineeringIntent::Unknown | EngineeringIntent::Chat => {
            let guard = ai_state.0.lock().await;

            if let Some(shared_orch) = &*guard {
                let mut orchestrator = shared_orch.lock().await;
                match orchestrator.ask(&user_input).await {
                    Ok(response_text) => Ok(Some(AgentResult::text(response_text))),
                    Err(e) => Err(e),
                }
            } else {
                Ok(Some(AgentResult::text(
                    "⏳ L'IA est en cours d'initialisation...".to_string(),
                )))
            }
        }

        _ => Ok(Some(AgentResult::text("Commande non gérée.".to_string()))),
    };

    match result {
        Ok(Some(res)) => Ok(res),
        Ok(None) => Ok(AgentResult::text("Aucune action effectuée.".to_string())),
        Err(e) => Err(format!("Erreur Agent : {}", e)),
    }
}

#[command]
pub async fn ask_native_llm(
    state: State<'_, NativeLlmState>,
    system_prompt: String,
    user_prompt: String,
) -> Result<String, String> {
    let mut guard = state
        .0
        .lock()
        .map_err(|_| "Erreur critique : Verrouillage du moteur impossible".to_string())?;

    if let Some(engine) = guard.as_mut() {
        match engine.generate(&system_prompt, &user_prompt, 1000) {
            Ok(response) => Ok(response),
            Err(e) => Err(format!("Erreur lors de la génération native : {}", e)),
        }
    } else {
        Err(
            "Le modèle IA natif est encore en cours de chargement... Veuillez patienter."
                .to_string(),
        )
    }
}

// --- COMMANDES DEEP LEARNING LEGACY ---

#[command]
pub fn init_dl_model(
    state: State<'_, DlState>,
    input_dim: usize,
    hidden_dim: usize,
    output_dim: usize,
) -> Result<String, String> {
    let device = Device::Cpu;
    let varmap = VarMap::new();
    let vb = VarBuilder::from_varmap(&varmap, DType::F32, &device);
    let model =
        SequenceNet::new(input_dim, hidden_dim, output_dim, vb).map_err(|e| e.to_string())?;
    let mut model_guard = state.model.lock().unwrap();
    let mut varmap_guard = state.varmap.lock().unwrap();
    *model_guard = Some(model);
    *varmap_guard = Some(varmap);
    Ok("Modèle Deep Learning initialisé.".to_string())
}

#[command]
pub fn run_dl_prediction(
    state: State<'_, DlState>,
    input_sequence: Vec<f32>,
) -> Result<Vec<f32>, String> {
    let model_guard = state.model.lock().unwrap();
    if let Some(model) = &*model_guard {
        let device = Device::Cpu;
        let input_dim = input_sequence.len();
        let input_tensor = Tensor::from_vec(input_sequence, (1, 1, input_dim), &device)
            .map_err(|e| e.to_string())?;
        let output = model.forward(&input_tensor).map_err(|e| e.to_string())?;
        let result_vec = output
            .flatten_all()
            .map_err(|e| e.to_string())?
            .to_vec1::<f32>()
            .map_err(|e| e.to_string())?;
        Ok(result_vec)
    } else {
        Err("Aucun modèle DL chargé.".to_string())
    }
}

#[command]
pub fn train_dl_step(
    state: State<'_, DlState>,
    input_sequence: Vec<f32>,
    target_class: u32,
) -> Result<f64, String> {
    let model_guard = state.model.lock().unwrap();
    let varmap_guard = state.varmap.lock().unwrap();
    if let (Some(model), Some(varmap)) = (&*model_guard, &*varmap_guard) {
        let device = Device::Cpu;
        let input_dim = input_sequence.len();
        let input_tensor = Tensor::from_vec(input_sequence, (1, 1, input_dim), &device)
            .map_err(|e| e.to_string())?;
        let target_tensor =
            Tensor::from_vec(vec![target_class], (1, 1), &device).map_err(|e| e.to_string())?;
        let trainer = Trainer::new(varmap, 0.01);
        let loss = trainer
            .train_step(model, &input_tensor, &target_tensor)
            .map_err(|e| e.to_string())?;
        Ok(loss)
    } else {
        Err("Modèle DL non chargé ou non entraînable.".to_string())
    }
}

#[command]
pub fn save_dl_model(state: State<'_, DlState>, path: String) -> Result<String, String> {
    let varmap_guard = state.varmap.lock().unwrap();
    if let Some(varmap) = &*varmap_guard {
        let path_buf = PathBuf::from(path);
        serialization::save_model(varmap, &path_buf).map_err(|e| e.to_string())?;
        Ok("Sauvegarde DL réussie.".to_string())
    } else {
        Err("Pas de modèle DL à sauvegarder.".to_string())
    }
}

#[command]
pub fn load_dl_model(
    state: State<'_, DlState>,
    path: String,
    input_dim: usize,
    hidden_dim: usize,
    output_dim: usize,
) -> Result<String, String> {
    let device = Device::Cpu;
    let path_buf = PathBuf::from(path);
    let model = serialization::load_model(&path_buf, input_dim, hidden_dim, output_dim, &device)
        .map_err(|e| e.to_string())?;
    let mut model_guard = state.model.lock().unwrap();
    let mut varmap_guard = state.varmap.lock().unwrap();
    *model_guard = Some(model);
    *varmap_guard = None;
    Ok("Modèle DL chargé.".to_string())
}
