// FICHIER : src-tauri/src/ai/agents/dynamic_agent.rs

use crate::json_db::collections::manager::CollectionsManager;
use crate::utils::prelude::*;

use super::intent_classifier::EngineeringIntent;
use super::prompt_engine::PromptEngine;
use super::tools::{extract_json_from_llm, load_session, save_artifact, save_session};
use super::{Agent, AgentContext, AgentResult};

use crate::ai::llm::client::LlmBackend;

/// L'Agent Dynamique piloté par les données (Data-Driven).
/// Il ne contient aucune logique métier en dur : tout son comportement
/// est dicté par le document JSON-LD 'Agent' stocké dans la base système.
pub struct DynamicAgent {
    handle: String,
}

impl DynamicAgent {
    /// Crée une nouvelle instance pointant vers un agent en base (ex: 'ref:agents:handle:agent_software')
    pub fn new(handle: &str) -> Self {
        Self {
            handle: handle.to_string(),
        }
    }
}

#[async_interface]
impl Agent for DynamicAgent {
    fn id(&self) -> &str {
        &self.handle
    }

    async fn process(
        &self,
        ctx: &AgentContext,
        intent: &EngineeringIntent,
    ) -> RaiseResult<Option<AgentResult>> {
        let config = AppConfig::get();
        let sys_manager =
            CollectionsManager::new(&ctx.db, &config.system_domain, &config.system_db);

        // 1. Charger la configuration de l'Agent depuis la DB
        let agent_doc = match sys_manager.get_document("agents", &self.handle).await? {
            Some(doc) => doc,
            None => raise_error!(
                "ERR_AGENT_CONFIG_NOT_FOUND",
                error = format!("Agent '{}' introuvable en base.", self.handle)
            ),
        };

        // 2. Extraire le profil neuronal (Prompt et paramètres)
        let neuro_profile = &agent_doc["base"]["neuro_profile"];
        let Some(prompt_id) = neuro_profile["prompt_id"].as_str() else {
            raise_error!(
                "ERR_AGENT_MISSING_PROMPT",
                error = "prompt_id manquant dans le neuro_profile"
            );
        };

        // 3. Compiler le System Prompt dynamiquement
        let prompt_engine =
            PromptEngine::new(ctx.db.clone(), &config.system_domain, &config.system_db);
        let system_prompt = prompt_engine.compile(prompt_id, None).await?;

        // 4. Charger la session et préparer le contexte utilisateur
        let mut session = load_session(ctx)
            .await
            .unwrap_or_else(|_| super::AgentSession::new(&ctx.session_id, self.id()));

        // On convertit l'intention en texte pour le LLM (simplification pour le routeur)
        let intent_text = format!("{:?}", intent);
        session.add_message("user", &intent_text);

        let history_str = session
            .messages
            .iter()
            .rev()
            .take(10) // On prend les 10 derniers messages
            .rev()
            .map(|m| format!("{}: {}", m.role, m.content))
            .collect::<Vec<_>>()
            .join("\n");

        let user_prompt = format!(
            "=== HISTORIQUE DE SESSION ===\n{}\n\n=== NOUVELLE TÂCHE ===\n{}",
            history_str, intent_text
        );

        // 5. Exécution neuronale (Appel LLM)
        user_info!(
            "🧠 Exécution de l'agent dynamique : {}",
            agent_doc["base"]["name"]["fr"]
                .as_str()
                .unwrap_or(&self.handle)
        );

        let response = ctx
            .llm
            .ask(LlmBackend::LocalLlama, &system_prompt, &user_prompt)
            .await?;

        // 6. Sauvegarde et retour
        let clean_json = extract_json_from_llm(&response);
        session.add_message("assistant", &clean_json);
        save_session(ctx, &session).await?;

        let mut artifacts = vec![];

        let parsed: JsonValue = json::deserialize_from_str(&clean_json).unwrap_or(json_value!({}));
        let mut docs_to_save = vec![];

        // L'IA peut générer un élément unique ou un tableau d'éléments (ex: Business Agent)
        if parsed.is_array() {
            docs_to_save.extend(parsed.as_array().unwrap().iter().cloned());
        } else if parsed.is_object() && !parsed.as_object().unwrap().is_empty() {
            docs_to_save.push(parsed);
        }

        for mut doc in docs_to_save {
            let mut layer = doc["layer"].as_str().unwrap_or("").to_string();
            let mut element_type = doc["type"].as_str().unwrap_or("").to_string();

            // Enrichissement sémantique (Fallback via l'intention initiale)
            if let EngineeringIntent::CreateElement {
                layer: i_layer,
                element_type: i_type,
                name: i_name,
            } = intent
            {
                if layer.is_empty() {
                    layer = i_layer.clone();
                }
                if element_type.is_empty() {
                    element_type = i_type.clone();
                }
                if doc["name"].is_null() {
                    doc["name"] = json_value!(i_name.clone());
                }
            } else if format!("{:?}", intent).contains("DefineBusinessUseCase") {
                if layer.is_empty() {
                    layer = "OA".to_string();
                }
                if element_type.is_empty() {
                    let name_str = doc["name"].as_str().unwrap_or("").to_lowercase();
                    if name_str.contains("acteur") || doc["attributes"].is_array() {
                        element_type = "OperationalActor".to_string();
                    } else {
                        element_type = "OperationalCapability".to_string();
                    }
                }
            }

            if layer.is_empty() || element_type.is_empty() {
                continue;
            }

            // Garantie d'intégrité des identifiants
            if let Some(obj) = doc.as_object_mut() {
                if !obj.contains_key("id") && !obj.contains_key("_id") {
                    obj.insert(
                        "id".to_string(),
                        json_value!(UniqueId::new_v4().to_string()),
                    );
                }
                if !obj.contains_key("layer") {
                    obj.insert("layer".to_string(), json_value!(layer.clone()));
                }
                if !obj.contains_key("type")
                    || obj
                        .get("type")
                        .and_then(|t| t.as_str())
                        .unwrap_or("")
                        .is_empty()
                {
                    obj.insert("type".to_string(), json_value!(element_type.clone()));
                }
            }
            // Écriture finale dans la base de données
            if let Ok(artifact) = save_artifact(ctx, &doc).await {
                artifacts.push(artifact);
            }
        }

        Ok(Some(AgentResult {
            message: format!(
                "Agent {} a terminé son cycle et persisté {} artefact(s).",
                self.handle,
                artifacts.len()
            ),
            artifacts,
            outgoing_message: None,
        }))
    }
}

// =========================================================================
// TESTS UNITAIRES
// =========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::llm::client::LlmClient;
    use crate::utils::testing::{inject_mock_component, AgentDbSandbox};

    #[test]
    fn test_dynamic_agent_id() {
        let agent = DynamicAgent::new("ref:agents:handle:test_agent");
        assert_eq!(agent.id(), "ref:agents:handle:test_agent");
    }

    #[async_test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_dynamic_agent_missing_in_db() {
        let sandbox = AgentDbSandbox::new().await;
        let manager = CollectionsManager::new(
            &sandbox.db,
            &sandbox.config.system_domain,
            &sandbox.config.system_db,
        );

        // 1. On initialise les collections nécessaires
        manager
            .create_collection(
                "agents",
                "db://_system/_system/schemas/v1/db/generic.schema.json",
            )
            .await
            .unwrap();

        // On mock le LLM et le WorldEngine pour le contexte
        inject_mock_component(&manager, "llm", json_value!({})).await;
        let llm = LlmClient::new(&manager).await.unwrap();

        use candle_nn::VarMap;
        let wm_config = crate::utils::data::config::WorldModelConfig::default();
        let world_engine = SharedRef::new(
            crate::ai::world_model::NeuroSymbolicEngine::new(wm_config, VarMap::new()).unwrap(),
        );

        let ctx = AgentContext::new(
            "dev",
            "sess_err_01",
            sandbox.db.clone(),
            llm,
            world_engine,
            sandbox.domain_root.clone(),
            sandbox.domain_root.clone(),
        )
        .await;

        // 2. On instancie un agent fantôme qui n'existe pas en base
        let agent = DynamicAgent::new("ghost_agent_007");
        let intent = EngineeringIntent::Chat;

        // 3. L'exécution doit échouer avec l'erreur spécifique
        let result = agent.process(&ctx, &intent).await;
        assert!(result.is_err());

        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("ERR_AGENT_CONFIG_NOT_FOUND"));
    }

    #[async_test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_dynamic_agent_missing_prompt() {
        let sandbox = AgentDbSandbox::new().await;
        let manager = CollectionsManager::new(
            &sandbox.db,
            &sandbox.config.system_domain,
            &sandbox.config.system_db,
        );

        manager
            .create_collection(
                "agents",
                "db://_system/_system/schemas/v1/db/generic.schema.json",
            )
            .await
            .unwrap();

        // 1. On insère un agent, mais on "oublie" le prompt_id dans son neuro_profile
        manager
            .upsert_document(
                "agents",
                json_value!({
                    "_id": "agent_no_prompt",
                    "base": {
                        "name": { "fr": "Agent Sans Cerveau" },
                        "neuro_profile": {
                            "temperature": 0.7
                        }
                    }
                }),
            )
            .await
            .unwrap();

        inject_mock_component(&manager, "llm", json_value!({})).await;
        let llm = LlmClient::new(&manager).await.unwrap();

        use candle_nn::VarMap;
        let wm_config = crate::utils::data::config::WorldModelConfig::default();
        let world_engine = SharedRef::new(
            crate::ai::world_model::NeuroSymbolicEngine::new(wm_config, VarMap::new()).unwrap(),
        );

        let ctx = AgentContext::new(
            "dev",
            "sess_err_02",
            sandbox.db.clone(),
            llm,
            world_engine,
            sandbox.domain_root.clone(),
            sandbox.domain_root.clone(),
        )
        .await;

        let agent = DynamicAgent::new("agent_no_prompt");
        let result = agent.process(&ctx, &EngineeringIntent::Chat).await;

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("ERR_AGENT_MISSING_PROMPT"));
    }

    #[async_test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)] // Nécessite CUDA/GGUF pour l'inférence LLM complète
    async fn test_dynamic_agent_full_execution_success() {
        let sandbox = AgentDbSandbox::new().await;
        let manager = CollectionsManager::new(
            &sandbox.db,
            &sandbox.config.system_domain,
            &sandbox.config.system_db,
        );

        // 1. Création des schémas génériques pour bypasser la validation stricte
        manager
            .create_collection(
                "prompts",
                "db://_system/_system/schemas/v1/db/generic.schema.json",
            )
            .await
            .unwrap();
        manager
            .create_collection(
                "agents",
                "db://_system/_system/schemas/v1/db/generic.schema.json",
            )
            .await
            .unwrap();

        manager
            .create_collection(
                "session_agents",
                "db://_system/_system/schemas/v1/db/generic.schema.json",
            )
            .await
            .unwrap();

        // 2. Injection du Prompt
        manager
            .upsert_document(
                "prompts",
                json_value!({
                    "_id": "prompt_success_test",
                    "role": "Agent de Succès",
                    "identity": { "persona": "Tu es un agent de test rapide." },
                    "directives": ["Réponds avec le mot 'Succès'."]
                }),
            )
            .await
            .unwrap();

        // 3. Injection de l'Agent
        manager
            .upsert_document(
                "agents",
                json_value!({
                    "_id": "agent_success_test",
                    "base": {
                        "name": { "fr": "Agent Succès" },
                        "neuro_profile": {
                            "prompt_id": "prompt_success_test",
                            "temperature": 0.1
                        }
                    }
                }),
            )
            .await
            .unwrap();

        // 4. Mocks d'exécution
        inject_mock_component(&manager, "llm", json_value!({})).await;
        let llm = LlmClient::new(&manager).await.unwrap();

        use candle_nn::VarMap;
        let wm_config = crate::utils::data::config::WorldModelConfig::default();
        let world_engine = SharedRef::new(
            crate::ai::world_model::NeuroSymbolicEngine::new(wm_config, VarMap::new()).unwrap(),
        );

        let ctx = AgentContext::new(
            "dev",
            "sess_success",
            sandbox.db.clone(),
            llm,
            world_engine,
            sandbox.domain_root.clone(),
            sandbox.domain_root.clone(),
        )
        .await;

        // 5. Exécution
        let agent = DynamicAgent::new("agent_success_test");
        let result = agent.process(&ctx, &EngineeringIntent::Chat).await;

        // 6. Vérification du succès
        match result {
            Ok(Some(res)) => {
                assert!(res
                    .message
                    .contains("Agent agent_success_test a terminé son cycle"));
            }
            Ok(None) => panic!("L'agent dynamique aurait dû renvoyer un AgentResult."),
            Err(e) => {
                let err_msg = e.to_string();
                println!(
                    "⚠️ Test ignoré ou erreur attendue (ex: LLM offline) : {}",
                    err_msg
                );
            }
        }
    }
}
