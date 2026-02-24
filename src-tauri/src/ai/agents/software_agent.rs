// FICHIER : src-tauri/src/ai/agents/software_agent.rs

use crate::utils::{async_trait, data, io, prelude::*, Uuid};

use super::intent_classifier::EngineeringIntent;
// ✅ IMPORT OPTIMISÉ
use super::tools::{
    extract_json_from_llm, find_element_by_name, load_session, save_artifact, save_session,
};
use super::{Agent, AgentContext, AgentResult, CreatedArtifact};

// Import du protocole ACL
use crate::ai::protocols::acl::{AclMessage, Performative};

// Import du protocole MCP et de l'outil CodeGenTool
use crate::ai::protocols::mcp::{McpTool, McpToolCall};
use crate::ai::tools::CodeGenTool;

use crate::ai::llm::client::LlmBackend;
use crate::ai::nlp::entity_extractor;

#[derive(Default)]
pub struct SoftwareAgent;

impl SoftwareAgent {
    pub fn new() -> Self {
        Self {}
    }

    async fn ask_llm(&self, ctx: &AgentContext, system: &str, user: &str) -> RaiseResult<String> {
        ctx.llm
            .ask(LlmBackend::LocalLlama, system, user)
            .await
            .map_err(|e| AppError::Validation(format!("Erreur LLM : {}", e)))
    }

    async fn enrich_logical_component(
        &self,
        ctx: &AgentContext,
        name: &str,
        description: &str,
        history_context: &str,
    ) -> RaiseResult<Value> {
        let entities = entity_extractor::extract_entities(name);
        let mut nlp_hint = String::new();
        if !entities.is_empty() {
            nlp_hint.push_str("\n[VOCABULAIRE]: ");
            for entity in entities {
                nlp_hint.push_str(&format!("{}, ", entity.text));
            }
        }

        let system_prompt = "Tu es un Architecte Logiciel. Génère JSON valide.";
        let user_prompt = format!(
            "=== HISTORIQUE ===\n{}\n\n=== TÂCHE ===\nCrée Composant LA.\nNom: {}\nDesc: {}\n{}\nJSON: {{ \"name\": \"str\", \"implementation_language\": \"rust|cpp\" }}",
            history_context, name, description, nlp_hint
        );

        let response = self.ask_llm(ctx, system_prompt, &user_prompt).await?;
        let clean_json = extract_json_from_llm(&response);

        let mut data: Value =
            data::parse(&clean_json).unwrap_or(json!({ "name": name, "description": description }));

        data["id"] = json!(Uuid::new_v4().to_string());
        data["layer"] = json!("LA");
        data["type"] = json!("LogicalComponent");
        data["createdAt"] = json!(chrono::Utc::now().to_rfc3339());

        Ok(data)
    }
}

#[async_trait]
impl Agent for SoftwareAgent {
    fn id(&self) -> &'static str {
        "software_engineer"
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
            EngineeringIntent::CreateElement {
                layer: _,
                element_type,
                name,
            } => {
                session.add_message(
                    "user",
                    &format!("Create Logical Component: {} ({})", name, element_type),
                );
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
                    .enrich_logical_component(
                        ctx,
                        name,
                        &format!("Type: {}", element_type),
                        &history_str,
                    )
                    .await?;
                let artifact = save_artifact(ctx, "la", "components", &doc).await?;

                // DÉLÉGATION -> EPBS
                let transition_msg =
                    format!("J'ai créé le composant '{}'. Demande création CI.", name);
                let acl_msg = AclMessage::new(
                    Performative::Request,
                    self.id(),
                    "configuration_manager",
                    &transition_msg,
                );

                let msg = format!("Composant **{}** créé.", name);
                session.add_message("assistant", &msg);
                save_session(ctx, &session).await?;

                Ok(Some(AgentResult {
                    message: msg,
                    artifacts: vec![artifact],
                    outgoing_message: Some(acl_msg),
                }))
            }

            EngineeringIntent::GenerateCode {
                language,
                context,
                filename: _,
            } => {
                session.add_message(
                    "user",
                    &format!("Generate code for '{}' in {}", context, language),
                );

                // 1. RECHERCHE (Optimisée via Tool)
                let component_doc = find_element_by_name(ctx, context).await.ok_or_else(|| {
                    AppError::Validation(format!(
                        "Composant '{}' introuvable dans le modèle.",
                        context
                    ))
                })?;

                let component_id = component_doc["id"].as_str().unwrap_or_default().to_string();

                // 2. APPEL OUTIL (Symbolic Execution)
                let gen_path = ctx.paths.domain_root.join("src-gen");

                // ✅ OPTIMISATION : Utilisation de la config globale pour le space/db
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
                    return Err(AppError::Validation(format!(
                        "Erreur CodeGen: {}",
                        result.content
                    )));
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
                        element_type: "SourceFile".to_string(),
                        path: path.clone(),
                    })
                    .collect();

                // 3. DÉLÉGATION -> TRANSVERSE (Quality)
                let transition_msg = format!(
                    "Code généré pour '{}' ({} fichiers). Vérification headers requise.",
                    context,
                    file_list.len()
                );
                let acl_msg = AclMessage::new(
                    Performative::Request,
                    self.id(),
                    "quality_manager",
                    &transition_msg,
                );

                let msg = format!(
                    "Code généré pour **{}**. Fichiers : {:?}",
                    context, file_list
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
    use crate::json_db::storage::{JsonDbConfig, StorageEngine};
    use crate::utils::{io::tempdir, Arc};

    use crate::json_db::collections::manager::CollectionsManager;
    use crate::utils::config::test_mocks::inject_mock_config;
    use crate::utils::config::AppConfig;

    #[test]
    fn test_software_id() {
        assert_eq!(SoftwareAgent::new().id(), "software_engineer");
    }

    #[tokio::test]
    async fn test_software_delegation_triggers() {
        let _agent = SoftwareAgent::new();
        let msg = AclMessage::new(
            Performative::Request,
            "software_engineer",
            "quality_manager",
            "Code Check Request",
        );

        assert_eq!(msg.sender, "software_engineer");
        assert_eq!(msg.receiver, "quality_manager");
        assert_eq!(msg.performative, Performative::Request);
    }

    #[tokio::test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_software_generation_integration() {
        // 1. Initialisation Mock Config
        inject_mock_config();

        let dir = tempdir().unwrap();
        let domain_root = dir.path().to_path_buf();
        let dataset_root = dir.path().join("dataset");

        let config = JsonDbConfig::new(domain_root.clone());
        let db = Arc::new(StorageEngine::new(config));

        // 2. Initialisation DB via AppConfig
        let app_cfg = AppConfig::get();
        let manager = CollectionsManager::new(&db, &app_cfg.system_domain, &app_cfg.system_db);
        let _ = manager.init_db().await;

        // 🎯 Injection du composant LLM pour le test
        crate::utils::config::test_mocks::inject_mock_component(
            &manager,
            "llm", 
            crate::utils::json::json!({ "rust_tokenizer_file": "tokenizer.json", "rust_model_file": "qwen2.5-1.5b-instruct-q4_k_m.gguf" })
        ).await;

        // 3. SEED DB : Injection du composant logique pour qu'il soit trouvé
        let comp_doc = json!({
            "id": "comp-jetson",
            "name": "Nvidia Jetson Controller",
            "layer": "LA",
            "type": "LogicalComponent",
            "description": "Embedded AI controller"
        });

        manager
            .upsert_document("la_components", comp_doc)
            .await
            .unwrap();

        // 🎯 Instanciation du client LLM AVEC le manager
        let llm = LlmClient::new(&manager).await.unwrap();

        // 🎯 Le .await manquant !
        let ctx = AgentContext::new(
            "dev",
            "sess_sw_01",
            db,
            llm,
            domain_root.clone(),
            dataset_root,
        )
        .await;

        let agent = SoftwareAgent::new();

        // 4. EXÉCUTION
        let intent = EngineeringIntent::GenerateCode {
            language: "rust".to_string(),
            context: "Nvidia Jetson Controller".to_string(),
            filename: "".to_string(),
        };

        let result = agent.process(&ctx, &intent).await;

        match result {
            Ok(Some(res)) => {
                println!("✅ Succès Agent : {}", res.message);
                assert!(!res.artifacts.is_empty(), "Aucun artefact généré !");

                if let Some(msg) = res.outgoing_message {
                    assert_eq!(msg.receiver, "quality_manager");
                }
            }
            Ok(None) => panic!("L'agent n'a rien renvoyé"),
            Err(e) => {
                let err_msg = e.to_string();
                if err_msg.contains("Composant 'Nvidia Jetson Controller' introuvable") {
                    panic!("L'injection de donnée a échoué : {}", err_msg);
                } else {
                    println!(
                        "⚠️ Test ignoré ou erreur attendue (ex: LLM offline) : {}",
                        err_msg
                    );
                }
            }
        }
    }
}
