use crate::ai::agents::intent_classifier::{EngineeringIntent, IntentClassifier};
use crate::ai::agents::{system_agent::SystemAgent, Agent};
use crate::ai::context::retriever::SimpleRetriever;
use crate::ai::llm::client::{LlmBackend, LlmClient};
use crate::json_db::storage::StorageEngine;
use crate::model_engine::loader::ModelLoader;

use std::env;
use tauri::{command, State};

#[command]
pub async fn ai_chat(
    storage: State<'_, StorageEngine>,
    user_input: String,
) -> Result<String, String> {
    // 1. CONFIGURATION
    let mode_dual =
        env::var("GENAPTITUDE_MODE_DUAL").unwrap_or_else(|_| "false".to_string()) == "true";
    let gemini_key = env::var("GENAPTITUDE_GEMINI_KEY").unwrap_or_default();
    let model_name = env::var("GENAPTITUDE_MODEL_NAME").ok();
    let local_url =
        env::var("GENAPTITUDE_LOCAL_URL").unwrap_or_else(|_| "http://localhost:8080".to_string());

    let client = LlmClient::new(&local_url, &gemini_key, model_name.clone());
    let classifier = IntentClassifier::new(client.clone());

    println!("🧠 Analyse de l'intention...");
    let intent = classifier.classify(&user_input).await;

    // 2. ROUTAGE DE L'INTENTION
    match intent {
        // CAS A : CRÉATION D'ÉLÉMENT (Acteur, Fonction...)
        // On utilise 'ref' pour ne pas consommer la variable 'intent' (ownership)
        EngineeringIntent::CreateElement { .. } => {
            let sys_agent = SystemAgent::new(client.clone(), storage.inner().clone());

            // On délègue à l'agent
            if let Some(result_msg) = sys_agent
                .process(&intent)
                .await
                .map_err(|e| e.to_string())?
            {
                return Ok(result_msg);
            }

            Ok("⚠️ Je n'ai pas trouvé d'agent compétent pour ce type d'élément.".to_string())
        }

        // CAS B : CRÉATION DE RELATION (Nouveau !)
        EngineeringIntent::CreateRelationship {
            ref source_name,
            ref target_name,
            ref relation_type,
        } => {
            let sys_agent = SystemAgent::new(client.clone(), storage.inner().clone());

            println!(
                "🔗 Tentative de liaison : {} -> {} ({})",
                source_name, target_name, relation_type
            );

            // On délègue à l'agent
            if let Some(result_msg) = sys_agent
                .process(&intent)
                .await
                .map_err(|e| e.to_string())?
            {
                Ok(result_msg)
            } else {
                // Fallback temporaire tant que SystemAgent::process ne gère pas ce cas
                Ok(format!(
                    "⚠️ J'ai bien compris que vous voulez lier **{}** et **{}**, mais je ne sais pas encore le faire techniquement. (En cours de dev)", 
                    source_name, target_name
                ))
            }
        }

        // CAS C : DISCUSSION / RAG (Consultation)
        EngineeringIntent::Chat | EngineeringIntent::Unknown => {
            println!("📂 Chargement du contexte métier pour RAG...");

            // 1. Chargement du modèle en mémoire (Thread blocking)
            let storage_clone = storage.inner().clone();
            let project_model = tauri::async_runtime::spawn_blocking(move || {
                let loader = ModelLoader::from_engine(&storage_clone, "un2", "_system");
                loader.load_full_model()
            })
            .await
            .map_err(|e| format!("Erreur thread: {}", e))?
            .map_err(|e| format!("Erreur chargement modèle: {}", e))?;

            // 2. Recherche contextuelle
            let retriever = SimpleRetriever::new(project_model);
            let context_data = retriever.retrieve_context(&user_input);

            // 3. Choix du backend (Dual Mode)
            let use_cloud = mode_dual && !gemini_key.is_empty() && is_complex_task(&user_input);
            let (backend, display_name) = if use_cloud {
                let name = model_name.unwrap_or_else(|| "Gemini Pro".to_string());
                (LlmBackend::GoogleGemini, format!("☁️ {} (Cloud)", name))
            } else {
                (LlmBackend::LocalLlama, "🏠 Mistral (Local)".to_string())
            };

            // 4. Prompt Système Augmenté
            let system_prompt = format!(
                "Tu es GenAptitude, expert en ingénierie système (Arcadia).
                 Utilise le contexte ci-dessous pour répondre précisément sur le projet en cours.
                 
                 CONTEXTE:
                 {}
                 ",
                context_data
            );

            // 5. Appel LLM
            let response = client
                .ask(backend, &system_prompt, &user_input)
                .await
                .map_err(|e| format!("Erreur IA: {}", e))?;

            Ok(format!("**{}**\n\n{}", display_name, response))
        }
    }
}

/// Helper pour détecter si la requête nécessite un LLM Cloud puissant
fn is_complex_task(input: &str) -> bool {
    let keywords = [
        "sml",
        "architecture",
        "analyse",
        "complexe",
        "génère",
        "entrainement",
        "synthèse",
    ];
    let input_lower = input.to_lowercase();
    keywords.iter().any(|&k| input_lower.contains(k))
}
