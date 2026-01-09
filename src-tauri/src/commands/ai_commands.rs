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
use tokio::sync::Mutex; // Async Mutex

use std::env;
use std::path::PathBuf;
use std::sync::Arc; // Standard Arc
use tauri::{command, State};

// --- DÉFINITION DE L'ÉTAT GLOBAL (PROPRIÉTÉ PARTAGÉE) ---
// On utilise une structure tuple pour envelopper le Mutex
// Type = Un Mutex qui contient (peut-être) un Pointeur Partagé vers un Mutex qui contient l'Orchestrateur
pub struct AiState(pub Mutex<Option<Arc<Mutex<AiOrchestrator>>>>);

impl AiState {
    pub fn new(orch: Option<Arc<Mutex<AiOrchestrator>>>) -> Self {
        Self(Mutex::new(orch))
    }
}

#[command]
pub async fn ai_reset(ai_state: State<'_, AiState>) -> Result<(), String> {
    // 1. On verrouille le conteneur principal
    let guard = ai_state.0.lock().await;

    // 2. Si l'IA est initialisée, on accède au pointeur partagé
    if let Some(shared_orch) = &*guard {
        // 3. On verrouille l'orchestrateur lui-même
        let mut orchestrator = shared_orch.lock().await;
        orchestrator.clear_history().map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Commande principale : Retourne un AgentResult structuré
#[command]
pub async fn ai_chat(
    storage: State<'_, StorageEngine>,
    ai_state: State<'_, AiState>,
    user_input: String,
) -> Result<AgentResult, String> {
    // 1. Configuration
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

    // 2. Classification
    let classifier = IntentClassifier::new(client.clone());
    let intent = classifier.classify(&user_input).await;

    let ctx = AgentContext::new(
        Arc::new(storage.inner().clone()),
        client.clone(),
        domain_path,
        dataset_path,
    );

    // 3. Routage
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
            // --- ACCÈS À L'ORCHESTRATEUR VIA ARC ---
            let guard = ai_state.0.lock().await;

            if let Some(shared_orch) = &*guard {
                // On verrouille l'instance partagée pour lui parler
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

    // 4. Conversion finale
    match result {
        Ok(Some(res)) => Ok(res),
        Ok(None) => Ok(AgentResult::text("Aucune action effectuée.".to_string())),
        Err(e) => Err(format!("Erreur Agent : {}", e)),
    }
}
