// FICHIER : src-tauri/src/ai/agents/software_agent.rs

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde_json::json;
use uuid::Uuid;

use super::intent_classifier::EngineeringIntent;
use super::tools::{extract_json_from_llm, load_session, save_artifact, save_session};
use super::{Agent, AgentContext, AgentResult, CreatedArtifact};

// Import du protocole ACL
use crate::ai::protocols::acl::{AclMessage, Performative};

// Import du protocole MCP et de l'outil CodeGenTool
use crate::ai::protocols::mcp::{McpTool, McpToolCall};
use crate::ai::tools::CodeGenTool;

use crate::ai::llm::client::LlmBackend;
use crate::ai::nlp::entity_extractor;

// Imports pour la recherche (Smart Linking)
use crate::json_db::collections::manager::CollectionsManager;
use crate::json_db::query::{Condition, FilterOperator, Query, QueryEngine, QueryFilter};

#[derive(Default)]
pub struct SoftwareAgent;

impl SoftwareAgent {
    pub fn new() -> Self {
        Self {}
    }

    async fn ask_llm(&self, ctx: &AgentContext, system: &str, user: &str) -> Result<String> {
        // En mode test, si le LLM n'est pas dispo, on suppose que ctx.llm gère ou on pourrait mocker.
        ctx.llm
            .ask(LlmBackend::LocalLlama, system, user)
            .await
            .map_err(|e| anyhow!("Erreur LLM : {}", e))
    }

    /// Tente de retrouver l'UUID d'un composant par son nom
    async fn find_component_id(&self, ctx: &AgentContext, name: &str) -> Option<String> {
        // Cible l'espace "mbse2/drones" utilisé lors de l'import CLI
        let manager = CollectionsManager::new(&ctx.db, "mbse2", "drones");
        let query_engine = QueryEngine::new(&manager);

        // Recherche dans les collections probables
        let collections = ["pa_components", "la_components", "sa_components"];

        for col in collections {
            let mut query = Query::new(col);
            query.filter = Some(QueryFilter {
                operator: FilterOperator::And,
                conditions: vec![Condition::eq("name", name.into())],
            });
            query.limit = Some(1);

            if let Ok(result) = query_engine.execute_query(query).await {
                if let Some(doc) = result.documents.first() {
                    // On cherche l'ID système (champ "id" ou "_id" selon la projection)
                    if let Some(id) = doc.get("id").and_then(|v| v.as_str()) {
                        return Some(id.to_string());
                    }
                }
            }
        }
        None
    }

    async fn enrich_logical_component(
        &self,
        ctx: &AgentContext,
        name: &str,
        description: &str,
        history_context: &str,
    ) -> Result<serde_json::Value> {
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

        let mut data: serde_json::Value = serde_json::from_str(&clean_json)
            .unwrap_or(json!({ "name": name, "description": description }));

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
    ) -> Result<Option<AgentResult>> {
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

                let artifact = save_artifact(ctx, "la", "components", &doc)?;

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
                context, // Nom du composant (ex: "Nvidia Jetson Controller")
                filename: _,
            } => {
                session.add_message(
                    "user",
                    &format!("Generate code for '{}' in {}", context, language),
                );

                // 1. RECHERCHE (Neuro-Symbolic)
                let component_id = self.find_component_id(ctx, context).await.ok_or_else(|| {
                    anyhow!("Composant '{}' introuvable dans mbse2/drones.", context)
                })?;

                // 2. APPEL OUTIL (Symbolic Execution)
                // CORRECTION : On cible le sous-dossier 'src-gen' pour ne pas polluer la racine
                let gen_path = ctx.paths.domain_root.join("src-gen");

                let tool = CodeGenTool::new(
                    gen_path,
                    ctx.db.clone(),
                    "mbse2",  // Espace cible
                    "drones", // Base cible
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
                    return Err(anyhow!("Erreur CodeGen: {}", result.content));
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
                        name: std::path::Path::new(path)
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
    use std::path::PathBuf;
    use std::sync::Arc;

    #[test]
    fn test_software_agent_id() {
        assert_eq!(SoftwareAgent::new().id(), "software_engineer");
    }

    #[tokio::test]
    async fn test_software_delegation_triggers() {
        let _agent = SoftwareAgent::new();

        // 1. Test Composant -> EPBS
        let msg_comp = AclMessage::new(
            Performative::Request,
            "software_engineer",
            "configuration_manager",
            "Create CI",
        );
        assert_eq!(msg_comp.receiver, "configuration_manager");

        // 2. Test Code -> Quality
        let msg_code = AclMessage::new(
            Performative::Request,
            "software_engineer",
            "quality_manager",
            "Create Tests",
        );
        assert_eq!(msg_code.receiver, "quality_manager");
    }

    // --- TEST D'INTÉGRATION COMPLET ---
    // Ce test simule le flux réel : Agent -> DB -> CodeGenTool -> Disque
    #[tokio::test]
    async fn test_generation_jetson_integration() {
        // Setup contexte réel vers la DB peuplée
        let domain_root = PathBuf::from("/home/zair/raise_domain");
        let dataset_root = PathBuf::from("/home/zair/raise_dataset");

        let config = JsonDbConfig::new(domain_root.clone());
        let db = Arc::new(StorageEngine::new(config));

        // Mock LLM
        let llm = LlmClient::new("http://localhost:11434", "dummy", None);

        let ctx = AgentContext::new(
            "test_user",
            "sess_integration_01",
            db,
            llm,
            domain_root,
            dataset_root,
        );

        let agent = SoftwareAgent::new();

        // Commande : Génère le code pour Nvidia Jetson Controller
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
                assert!(res.artifacts.iter().any(|a| a.path.contains("Cargo.toml")));

                // Vérif ACL sortant vers Quality Manager
                if let Some(msg) = res.outgoing_message {
                    assert_eq!(msg.receiver, "quality_manager");
                    println!("✅ Délégation envoyée vers Quality Manager");
                }
            }
            Ok(None) => panic!("L'agent n'a rien renvoyé"),
            Err(e) => panic!("L'agent a échoué : {:?}", e),
        }
    }
}
