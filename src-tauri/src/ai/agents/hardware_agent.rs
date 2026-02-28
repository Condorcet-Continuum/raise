// FICHIER : src-tauri/src/ai/agents/hardware_agent.rs

use crate::utils::{async_trait, data, io, prelude::*, Uuid};

use super::intent_classifier::EngineeringIntent;
// ✅ IMPORT OPTIMISÉ : On récupère l'outil de recherche centralisé
use super::tools::{
    extract_json_from_llm, find_element_by_name, load_session, save_artifact, save_session,
};
use super::{Agent, AgentContext, AgentResult, CreatedArtifact};

// IMPORTS PROTOCOLES
use crate::ai::protocols::acl::{AclMessage, Performative};
use crate::ai::protocols::mcp::{McpTool, McpToolCall};

// IMPORTS OUTILS & DB
use crate::ai::tools::CodeGenTool;

use crate::ai::llm::client::LlmBackend;
use crate::ai::nlp::entity_extractor;

#[derive(Default)]
pub struct HardwareAgent;

impl HardwareAgent {
    pub fn new() -> Self {
        Self {}
    }

    pub fn determine_category(&self, name: &str, element_type: &str) -> &'static str {
        let keywords = format!("{} {}", name, element_type).to_lowercase();
        if keywords.contains("fpga") || keywords.contains("asic") || keywords.contains("pcb") {
            "Electronics"
        } else {
            "Infrastructure"
        }
    }

    async fn enrich_physical_node(
        &self,
        ctx: &AgentContext,
        name: &str,
        element_type: &str,
        history_context: &str,
    ) -> RaiseResult<Value> {
        let category = self.determine_category(name, element_type);
        let instruction = if category == "Electronics" {
            "Contexte: Design Électronique (VHDL/Verilog)."
        } else {
            "Contexte: Infrastructure IT."
        };

        let entities = entity_extractor::extract_entities(name);
        let mut nlp_hint = String::new();
        if !entities.is_empty() {
            nlp_hint.push_str("\n[COMPOSANTS]: ");
            for entity in entities {
                nlp_hint.push_str(&format!("{}, ", entity.text));
            }
        }

        let system_prompt = "Tu es un Architecte Matériel. Génère JSON.";
        let user_prompt = format!(
            "=== HISTORIQUE ===\n{}\n\nCrée Noeud PA.\nNom: {}\nType: {}\n{}\n{}\nJSON: {{ \"name\": \"str\", \"specs\": {{}} }}",
            history_context, name, element_type, instruction, nlp_hint
        );

        let response = ctx
            .llm
            .ask(LlmBackend::LocalLlama, system_prompt, &user_prompt)
            .await?;

        let clean_json = extract_json_from_llm(&response);
        let mut data: Value = data::parse(&clean_json).unwrap_or(json!({ "name": name }));

        data["id"] = json!(Uuid::new_v4().to_string());
        data["layer"] = json!("PA");
        data["type"] = json!("PhysicalNode");
        data["nature"] = json!(category);
        data["createdAt"] = json!(chrono::Utc::now().to_rfc3339());

        Ok(data)
    }
}

#[async_trait]
impl Agent for HardwareAgent {
    fn id(&self) -> &'static str {
        "hardware_architect"
    }

    async fn process(
        &self,
        ctx: &AgentContext,
        intent: &EngineeringIntent,
    ) -> RaiseResult<Option<AgentResult>> {
        let mut session = load_session(ctx)
            .await
            .unwrap_or_else(|_| super::AgentSession::new(&ctx.session_id, &ctx.agent_id));

        match intent {
            // 1. CRÉATION (PA)
            EngineeringIntent::CreateElement {
                layer,
                element_type,
                name,
            } if layer == "PA" => {
                session.add_message("user", &format!("Create Node: {} ({})", name, element_type));

                let history_str = session
                    .messages
                    .iter()
                    .rev()
                    .take(5)
                    .rev()
                    .map(|m| format!("{}: {}", m.role, m.content))
                    .collect::<Vec<_>>()
                    .join("\n");

                let doc = self
                    .enrich_physical_node(ctx, name, element_type, &history_str)
                    .await?;
                let nature = doc["nature"].as_str().unwrap_or("Hardware").to_string();

                let artifact = save_artifact(ctx, "pa", "physical_nodes", &doc).await?;

                // Délégation -> EPBS
                let transition_msg = format!(
                    "J'ai spécifié le matériel '{}' (Nature: {}). Merci de créer l'Article de Configuration (CI) associé.",
                    name, nature
                );

                let acl_msg = AclMessage::new(
                    Performative::Request,
                    self.id(),
                    "configuration_manager",
                    &transition_msg,
                );

                let msg = format!(
                    "Noeud physique **{}** ({}) provisionné. Demande de création CI envoyée.",
                    name, nature
                );

                session.add_message("assistant", &msg);
                save_session(ctx, &session).await?;

                Ok(Some(AgentResult {
                    message: msg,
                    artifacts: vec![artifact],
                    outgoing_message: Some(acl_msg),
                }))
            }

            // 2. GÉNÉRATION CODE (VHDL/Verilog)
            EngineeringIntent::GenerateCode {
                language,
                context,
                filename: _,
            } => {
                session.add_message(
                    "user",
                    &format!("Generate hardware code for '{}' in {}", context, language),
                );

                // ✅ OPTIMISATION : Recherche via l'outil centralisé (Supporte PA, LA, SA)
                let Some(component_doc) = find_element_by_name(ctx, context).await else {
                    raise_error!(
                        "ERR_HW_COMPONENT_NOT_FOUND",
                        error = "Composant matériel introuvable dans le modèle monde",
                        context = serde_json::json!({ "requested_name": context })
                    );
                };

                let component_id = component_doc["id"].as_str().unwrap_or_default().to_string();

                // B. Appel Outil MCP
                let gen_path = ctx.paths.domain_root.join("src-gen");

                // ✅ OPTIMISATION : Utilisation de la config globale pour CodeGenTool
                let config = crate::utils::config::AppConfig::get();
                let tool = CodeGenTool::new(
                    gen_path,
                    ctx.db.clone(),
                    &config.system_domain, // ✅ CORRECTIF : system_domain
                    &config.system_db,     // ✅ CORRECTIF : system_db
                );

                let call = McpToolCall::new(
                    "generate_component_code",
                    json!({
                        "component_id": component_id,
                        "dry_run": false
                    }),
                );

                let result = tool.execute(call).await;

                if result.is_error {
                    raise_error!(
                        "ERR_HW_CODEGEN",
                        error = "Erreur CodeGen Hardware",
                        context = serde_json::json!({ "details": result.content })
                    );
                }

                let file_list = result.content["files"]
                    .as_array()
                    .map(|arr| {
                        arr.iter()
                            .map(|v| v.as_str().unwrap_or("?").to_string())
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();

                let artifacts: Vec<CreatedArtifact> = file_list
                    .iter()
                    .map(|path| CreatedArtifact {
                        id: format!("gen_{}", Uuid::new_v4()),
                        name: io::Path::new(path)
                            .file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .to_string(),
                        layer: "CODE".to_string(),
                        element_type: "HardwareSource".to_string(),
                        path: path.clone(),
                    })
                    .collect();

                // C. Délégation -> Quality
                let transition_msg = format!(
                    "Code HDL généré pour '{}' ({}). Vérification syntaxique requise.",
                    context, language
                );

                let acl_msg = AclMessage::new(
                    Performative::Request,
                    self.id(),
                    "quality_manager",
                    &transition_msg,
                );

                let msg = format!(
                    "Description matérielle ({}) générée pour **{}**. Fichiers : {:?}",
                    language, context, file_list
                );

                session.add_message("assistant", &msg);
                save_session(ctx, &session).await?;

                Ok(Some(AgentResult {
                    message: msg,
                    artifacts,
                    outgoing_message: Some(acl_msg),
                }))
            }

            _ => Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::llm::client::LlmClient;
    use crate::ai::protocols::acl::Performative;
    use crate::json_db::storage::{JsonDbConfig, StorageEngine};
    use crate::utils::{io::tempdir, Arc};

    use crate::json_db::collections::manager::CollectionsManager;
    use crate::utils::config::test_mocks::inject_mock_config;
    use crate::utils::config::AppConfig;

    #[test]
    fn test_category_detection() {
        let agent = HardwareAgent::new();
        assert_eq!(agent.determine_category("Carte Mère", "PCB"), "Electronics");
        assert_eq!(
            agent.determine_category("Serveur", "Rack"),
            "Infrastructure"
        );
    }

    #[tokio::test]
    async fn test_hardware_delegation_trigger() {
        let _agent = HardwareAgent::new();
        let msg = AclMessage::new(
            Performative::Request,
            "hardware_architect",
            "configuration_manager",
            "Content",
        );
        assert_eq!(msg.receiver, "configuration_manager");
    }

    #[tokio::test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_hardware_generation_integration() {
        inject_mock_config();

        let dir = tempdir().unwrap();
        let domain_root = dir.path().to_path_buf();

        let config = JsonDbConfig::new(domain_root.clone());
        let db = Arc::new(StorageEngine::new(config));

        let app_cfg = AppConfig::get();
        let manager = CollectionsManager::new(&db, &app_cfg.system_domain, &app_cfg.system_db);
        let _ = manager.init_db().await;

        // 🎯 Injection du composant LLM
        crate::utils::config::test_mocks::inject_mock_component(
            &manager,
            "llm", 
            crate::utils::json::json!({ "rust_tokenizer_file": "tokenizer.json", "rust_model_file": "qwen2.5-1.5b-instruct-q4_k_m.gguf" })
        ).await;

        let comp_doc = json!({
            "id": "fpga-001",
            "name": "FPGA Controller",
            "layer": "PA",
            "type": "PhysicalNode",
            "nature": "Electronics",
            "implementation": {
                "technology": "VHDL_Entity",
                "artifactName": "fpga_ctrl"
            }
        });
        manager
            .upsert_document("pa_components", comp_doc)
            .await
            .unwrap();

        // 🎯 .await sur LLM et AgentContext !
        let llm = LlmClient::new(&manager).await.unwrap();

        let ctx = AgentContext::new(
            "tester",
            "sess_hw_01",
            db,
            llm,
            domain_root.clone(),
            domain_root.clone(),
        )
        .await;

        let agent = HardwareAgent::new();

        let intent = EngineeringIntent::GenerateCode {
            language: "vhdl".into(),
            context: "FPGA Controller".into(),
            filename: "".into(),
        };

        let result = agent.process(&ctx, &intent).await;

        match result {
            Ok(Some(res)) => {
                println!("Output: {}", res.message);
                assert!(res.message.to_lowercase().contains("vhdl"));
                assert!(!res.artifacts.is_empty());
                if let Some(msg) = res.outgoing_message {
                    assert_eq!(msg.receiver, "quality_manager");
                } else {
                    panic!("Pas de message sortant vers la qualité");
                }
            }
            _ => panic!("Échec de la génération Hardware"),
        }
    }

    #[tokio::test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn generate_hardware_in_user_domain() {
        inject_mock_config();

        let app_config = AppConfig::get();
        let domain_root = app_config
            .get_path("PATH_RAISE_DOMAIN")
            .expect("PATH_RAISE_DOMAIN doit être défini");

        if !domain_root.exists() {
            std::fs::create_dir_all(&domain_root).unwrap();
        }

        let config = JsonDbConfig::new(domain_root.clone());
        let db = Arc::new(StorageEngine::new(config));

        let manager =
            CollectionsManager::new(&db, &app_config.system_domain, &app_config.system_db);
        let _ = manager.init_db().await;

        // 🎯 Injection du composant LLM
        crate::utils::config::test_mocks::inject_mock_component(
            &manager,
            "llm", 
            crate::utils::json::json!({ "rust_tokenizer_file": "tokenizer.json", "rust_model_file": "qwen2.5-1.5b-instruct-q4_k_m.gguf" })
        ).await;

        let comp_doc = json!({
            "id": "fpga-video-proc",
            "name": "Video Processor FPGA",
            "layer": "PA",
            "type": "PhysicalNode",
            "nature": "Electronics",
            "implementation": {
                "technology": "VHDL_Entity",
                "artifactName": "video_proc"
            }
        });
        manager
            .upsert_document("pa_components", comp_doc)
            .await
            .unwrap();

        // 🎯 .await sur LLM et AgentContext !
        let llm = LlmClient::new(&manager).await.unwrap();

        let ctx = AgentContext::new(
            "zair",
            "session_live",
            db,
            llm,
            domain_root.clone(),
            domain_root.clone(),
        )
        .await;

        let agent = HardwareAgent::new();

        let intent = EngineeringIntent::GenerateCode {
            language: "vhdl".into(),
            context: "Video Processor FPGA".into(),
            filename: "".into(),
        };

        let result = agent.process(&ctx, &intent).await.unwrap().unwrap();
        println!("✅ Résultat Agent : {}", result.message);

        assert!(result.message.contains("Video Processor FPGA"));
        assert!(!result.artifacts.is_empty());
    }
}
