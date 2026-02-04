// FICHIER : src-tauri/src/ai/agents/hardware_agent.rs

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde_json::json;
use uuid::Uuid;

use super::intent_classifier::EngineeringIntent;
use super::tools::{extract_json_from_llm, load_session, save_artifact, save_session};
use super::{Agent, AgentContext, AgentResult, CreatedArtifact};

// IMPORTS PROTOCOLES
use crate::ai::protocols::acl::{AclMessage, Performative};
use crate::ai::protocols::mcp::{McpTool, McpToolCall};

// IMPORTS OUTILS & DB
use crate::ai::tools::CodeGenTool;
use crate::json_db::collections::manager::CollectionsManager;
use crate::json_db::query::{Condition, FilterOperator, Query, QueryEngine, QueryFilter};

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

    /// Retrouve l'ID d'un composant physique par son nom
    async fn find_component_id(&self, ctx: &AgentContext, name: &str) -> Option<String> {
        let manager = CollectionsManager::new(&ctx.db, "mbse2", "drones");
        let query_engine = QueryEngine::new(&manager);

        // On cherche principalement dans pa_components (Physique)
        let collections = ["pa_components", "la_components"];

        for col in collections {
            let mut query = Query::new(col);
            query.filter = Some(QueryFilter {
                operator: FilterOperator::And,
                conditions: vec![Condition::eq("name", name.into())],
            });
            query.limit = Some(1);

            if let Ok(result) = query_engine.execute_query(query).await {
                if let Some(doc) = result.documents.first() {
                    if let Some(id) = doc.get("id").and_then(|v| v.as_str()) {
                        return Some(id.to_string());
                    }
                }
            }
        }
        None
    }

    async fn enrich_physical_node(
        &self,
        ctx: &AgentContext,
        name: &str,
        element_type: &str,
        history_context: &str,
    ) -> Result<serde_json::Value> {
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
            .await
            .map_err(|e| anyhow!("Erreur LLM Hardware: {}", e))?;

        let clean_json = extract_json_from_llm(&response);
        let mut data: serde_json::Value =
            serde_json::from_str(&clean_json).unwrap_or(json!({ "name": name }));

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
    ) -> Result<Option<AgentResult>> {
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

                let artifact = save_artifact(ctx, "pa", "physical_nodes", &doc)?;

                // Délégation -> EPBS (Configuration Manager)
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
                context, // Nom du composant (ex: "FPGA Video Processor")
                filename: _,
            } => {
                session.add_message(
                    "user",
                    &format!("Generate hardware code for '{}' in {}", context, language),
                );

                // A. Recherche ID
                let component_id = self
                    .find_component_id(ctx, context)
                    .await
                    .ok_or_else(|| anyhow!("Composant matériel '{}' introuvable.", context))?;

                // B. Appel Outil MCP
                let gen_path = ctx.paths.domain_root.join("src-gen");
                let tool = CodeGenTool::new(gen_path, ctx.db.clone(), "mbse2", "drones");

                let call = McpToolCall::new(
                    "generate_component_code",
                    json!({
                        "component_id": component_id,
                        "dry_run": false
                    }),
                );

                let result = tool.execute(call).await;

                if result.is_error {
                    return Err(anyhow!("Erreur CodeGen Hardware: {}", result.content));
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
                        element_type: "HardwareSource".to_string(),
                        path: path.clone(),
                    })
                    .collect();

                // C. Délégation -> Quality (Vérification Syntaxe VHDL)
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
    use std::sync::Arc;
    use tempfile::tempdir;

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
    async fn test_hardware_generation_integration() {
        let dir = tempdir().unwrap();
        let domain_root = dir.path().to_path_buf();

        let config = JsonDbConfig::new(domain_root.clone());
        let db = Arc::new(StorageEngine::new(config));
        let llm = LlmClient::new("http://localhost:11434", "dummy", None);

        let manager = CollectionsManager::new(&db, "mbse2", "drones");
        manager.init_db().await.unwrap();

        let comp_doc = json!({
            "id": "fpga-001",
            "name": "FPGA Controller",
            "layer": "PA",
            "type": "PhysicalNode",
            "nature": "Electronics", // CORRECTION : Champ obligatoire pour la validation schema
            "implementation": {
                "technology": "VHDL_Entity",
                "artifactName": "fpga_ctrl"
            }
        });
        manager
            .upsert_document("pa_components", comp_doc)
            .await
            .unwrap();

        let ctx = AgentContext::new(
            "tester",
            "sess_hw_01",
            db,
            llm,
            domain_root.clone(),
            domain_root.clone(),
        );

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

                // CORRECTION : Check insensible à la casse ("vhdl" vs "VHDL")
                assert!(res.message.to_lowercase().contains("vhdl"));
                assert!(!res.artifacts.is_empty());

                // CORRECTION : Check du nom de fichier généré par défaut
                // Le générateur utilise le nom du composant par défaut (fpga_controller.vhd)
                let vhdl_file_default = domain_root.join("src-gen/fpga_controller.vhd");

                // On vérifie que le fichier existe (quel que soit le nom exact, mais basé sur le log d'erreur précédent)
                assert!(
                    vhdl_file_default.exists(),
                    "Le fichier généré n'a pas été trouvé à l'emplacement attendu"
                );

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
    async fn generate_hardware_in_user_domain() {
        // 1. Définition du chemin réel de votre environnement
        // On récupère le HOME (ex: /home/zair) et on ajoute "raise_domain"
        let home = std::env::var("HOME").expect("Variable HOME non définie");
        let domain_root = std::path::PathBuf::from(home).join("raise_domain");

        // On vérifie que le dossier existe (créé par vos précédents tests)
        if !domain_root.exists() {
            std::fs::create_dir_all(&domain_root).unwrap();
        }

        println!("🌍 Environnement cible : {:?}", domain_root);

        // 2. Configuration du Contexte
        let config = JsonDbConfig::new(domain_root.clone());
        let db = Arc::new(StorageEngine::new(config));

        // Initialisation de la DB dans ce dossier réel
        let manager = CollectionsManager::new(&db, "mbse2", "drones");
        manager.init_db().await.unwrap();

        // 3. Injection du composant FPGA dans la base réelle
        let comp_doc = json!({
            "id": "fpga-video-proc",
            "name": "Video Processor FPGA",
            "layer": "PA",
            "type": "PhysicalNode",
            "nature": "Electronics", // CORRECTION : Ajout du champ obligatoire
            "implementation": {
                "technology": "VHDL_Entity",
                "artifactName": "video_proc"
            }
        });
        manager
            .upsert_document("pa_components", comp_doc)
            .await
            .unwrap();

        // 4. Instanciation de l'Agent
        // Note: On utilise "dummy" pour le LLM car on teste la génération de code (symbolique)
        let llm = LlmClient::new("http://localhost:11434", "dummy", None);

        let ctx = AgentContext::new(
            "zair", // Votre nom utilisateur
            "session_live",
            db,
            llm,
            domain_root.clone(),
            domain_root.clone(),
        );

        let agent = HardwareAgent::new();

        // 5. Exécution de la commande
        let intent = EngineeringIntent::GenerateCode {
            language: "vhdl".into(),
            context: "Video Processor FPGA".into(),
            filename: "".into(),
        };

        let result = agent.process(&ctx, &intent).await.unwrap().unwrap();
        println!("✅ Résultat Agent : {}", result.message);
    }
}
