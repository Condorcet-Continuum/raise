// FICHIER : src-tauri/src/ai/agents/transverse_agent.rs

use crate::utils::{async_trait, data, prelude::*, Uuid};

use super::intent_classifier::EngineeringIntent;
// Import des outils optimisés (plus besoin de CollectionsManager ici)
use super::tools::{
    extract_json_from_llm, find_element_by_name, load_session, save_artifact, save_session,
};
use super::{Agent, AgentContext, AgentResult};
use crate::ai::llm::client::LlmBackend;
use crate::ai::nlp::entity_extractor;

#[derive(Default)]
pub struct TransverseAgent;

impl TransverseAgent {
    pub fn new() -> Self {
        Self
    }

    async fn call_llm(
        &self,
        ctx: &AgentContext,
        sys: &str,
        user: &str,
        doc_type: &str,
        original_name: &str,
    ) -> RaiseResult<Value> {
        if original_name == "skip_llm" {
            return Ok(json!({}));
        }

        let response = ctx.llm.ask(LlmBackend::LocalLlama, sys, user).await?;

        let clean = extract_json_from_llm(&response);
        let mut doc: Value = data::parse(&clean).unwrap_or(json!({}));

        doc["name"] = json!(original_name);
        doc["id"] = json!(Uuid::new_v4().to_string());
        doc["layer"] = json!("TRANSVERSE");
        doc["type"] = json!(doc_type);
        doc["createdAt"] = json!(chrono::Utc::now().to_rfc3339());

        if doc_type == "Requirement" && doc.get("reqId").is_none() {
            doc["reqId"] = json!("REQ-AUTO");
        }

        Ok(doc)
    }

    async fn enrich_requirement(
        &self,
        ctx: &AgentContext,
        name: &str,
        history_context: &str,
    ) -> RaiseResult<Value> {
        let entities = entity_extractor::extract_entities(name);
        let mut nlp_hint = String::new();
        if !entities.is_empty() {
            nlp_hint.push_str("Concerne :\n");
            for e in entities {
                nlp_hint.push_str(&format!("- {}\n", e.text));
            }
        }
        let sys = "RÔLE: Ingénieur Exigences. JSON Strict.";
        let user = format!(
            "=== HISTORIQUE ===\n{}\n\nExigence: \"{}\"\n{}\nJSON: {{ \"statement\": \"str\", \"reqId\": \"REQ-01\" }}",
            history_context, name, nlp_hint
        );
        self.call_llm(ctx, sys, &user, "Requirement", name).await
    }

    /// Analyse statique : DB (via Tool) + FS
    async fn perform_static_analysis(
        &self,
        ctx: &AgentContext,
        target: &str,
    ) -> (bool, Vec<String>) {
        let mut findings = Vec::new();
        let mut overall_success = true;

        // 1. VÉRIFICATION MODÈLE (DB) - OPTIMISÉ
        findings.push(format!(
            "🔎 Recherche de '{}' dans la base de données...",
            target
        ));

        // ✅ Utilisation de l'outil centralisé (plus de code dupliqué pour la recherche)
        let db_component = find_element_by_name(ctx, target).await;

        if let Some(comp) = db_component {
            let id = comp.get("id").and_then(|v| v.as_str()).unwrap_or("?");
            let layer = comp.get("layer").and_then(|v| v.as_str()).unwrap_or("?");

            findings.push(format!(
                "✅ Composant trouvé en base (Layer: {}, ID: {})",
                layer, id
            ));

            // 2. VÉRIFICATION PHYSIQUE (FS)
            let target_snake = target.to_lowercase().replace(" ", "_");
            let src_gen = ctx.paths.domain_root.join("src-gen");
            let component_path = src_gen.join(&target_snake);

            if component_path.exists() {
                findings.push(format!(
                    "✅ Dossier source identifié : {:?}",
                    component_path
                ));

                if component_path.join("Cargo.toml").exists() {
                    findings.push("✅ Manifeste Cargo.toml trouvé".into());
                } else {
                    findings.push("⚠️ Cargo.toml manquant".into());
                    overall_success = false;
                }
            } else {
                findings.push(format!(
                    "❌ Dossier introuvable sur le disque : {:?}",
                    component_path
                ));
                findings.push("💡 Conseil : Demandez au Software Agent de générer le code.".into());
                overall_success = false;
            }
        } else {
            findings.push(format!("❌ Composant '{}' INCONNU dans le modèle.", target));
            findings.push("💡 Conseil : Créez d'abord le composant logique ou physique.".into());
            overall_success = false;
        }

        (overall_success, findings)
    }
}

#[async_trait]
impl Agent for TransverseAgent {
    fn id(&self) -> &'static str {
        "quality_manager"
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
                layer,
                element_type,
                name,
            } if layer == "TRANSVERSE" => {
                session.add_message(
                    "user",
                    &format!("Create Transverse: {} ({})", name, element_type),
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

                let (doc, sub_folder) = (
                    self.enrich_requirement(ctx, name, &history_str).await?,
                    "requirements",
                );

                let artifact = save_artifact(ctx, "transverse", sub_folder, &doc).await?;
                let msg = format!("Élément Transverse **{}** ({}) créé.", name, element_type);
                session.add_message("assistant", &msg);
                save_session(ctx, &session).await?;

                Ok(Some(AgentResult {
                    message: msg,
                    artifacts: vec![artifact],
                    outgoing_message: None,
                }))
            }

            EngineeringIntent::VerifyQuality { scope, target } => {
                session.add_message(
                    "user",
                    &format!("Verify Quality ({}) for {}", scope, target),
                );

                let (passed, findings) = self.perform_static_analysis(ctx, target).await;

                let report = json!({
                    "id": Uuid::new_v4().to_string(),
                    "type": "QualityAssessment",
                    "target": target,
                    "scope": scope,
                    "status": if passed { "PASSED" } else { "FAILED" },
                    "findings": findings,
                    "generatedAt": chrono::Utc::now().to_rfc3339()
                });

                let artifact = save_artifact(ctx, "transverse", "reports", &report).await?;
                let findings_list = findings
                    .iter()
                    .map(|f| format!("- {}", f))
                    .collect::<Vec<_>>()
                    .join("\n");
                let status_icon = if passed { "✅" } else { "❌" };

                let msg = format!(
                    "Rapport Qualité pour **{}**.\nStatut: {} **{}**\n\n{}",
                    target,
                    status_icon,
                    if passed { "CONFORME" } else { "NON CONFORME" },
                    findings_list
                );

                session.add_message("assistant", &msg);
                save_session(ctx, &session).await?;

                Ok(Some(AgentResult {
                    message: msg,
                    artifacts: vec![artifact],
                    outgoing_message: None,
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
    use crate::json_db::collections::manager::CollectionsManager;
    use crate::utils::io::{self};
    use crate::utils::json::json;
    use crate::utils::mock::{inject_mock_component, AgentDbSandbox};

    #[test]
    fn test_transverse_id() {
        assert_eq!(TransverseAgent::new().id(), "quality_manager");
    }

    #[tokio::test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_verify_quality_logic() {
        let sandbox = AgentDbSandbox::new().await;
        let manager = CollectionsManager::new(
            &sandbox.db,
            &sandbox.config.system_domain,
            &sandbox.config.system_db,
        );
        // 🎯 Injection du LLM Mocké
        inject_mock_component(
            &manager,
            "llm", 
            crate::utils::json::json!({ "rust_tokenizer_file": "tokenizer.json", "rust_model_file": "qwen2.5-1.5b-instruct-q4_k_m.gguf" })
        ).await;

        let comp_doc = json!({
            "id": "comp-123",
            "name": "Mon Composant",
            "layer": "LA",
            "type": "LogicalComponent"
        });

        manager
            .create_collection(
                "la_components",
                "db://_system/_system/schemas/v1/db/generic.schema.json",
            )
            .await
            .unwrap();
        manager
            .upsert_document("la_components", comp_doc)
            .await
            .unwrap();

        // 🎯 Client LLM et Context avec .await
        let llm = LlmClient::new(&manager).await.unwrap();
        let ctx = AgentContext::new(
            "tester",
            "sess_qual_01",
            sandbox.db,
            llm,
            sandbox.domain_root.clone(),
            sandbox.domain_root.clone(),
        )
        .await;

        let agent = TransverseAgent::new();

        let src_gen = sandbox.domain_root.join("src-gen").join("mon_composant");
        let full_path = src_gen.join("Cargo.toml");
        io::create_dir_all(&src_gen).await.unwrap();
        io::write(&full_path, "[package]\nname = \"mon_composant\"")
            .await
            .unwrap();

        let intent = EngineeringIntent::VerifyQuality {
            scope: "code".into(),
            target: "Mon Composant".into(),
        };

        let result = agent.process(&ctx, &intent).await;

        match result {
            Ok(Some(res)) => {
                println!("Output: {}", res.message);
                assert!(res.message.contains("CONFORME"));
                assert!(res.message.contains("Composant trouvé en base"));
                assert!(res.message.contains("Cargo.toml trouvé"));
            }
            _ => panic!("L'agent aurait dû retourner un résultat positif"),
        }
    }

    #[tokio::test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_verify_quality_missing_in_db() {
        let sandbox = AgentDbSandbox::new().await;
        let manager = CollectionsManager::new(
            &sandbox.db,
            &sandbox.config.system_domain,
            &sandbox.config.system_db,
        );
        inject_mock_component(
            &manager,
            "llm", 
            crate::utils::json::json!({ "rust_tokenizer_file": "tokenizer.json", "rust_model_file": "qwen2.5-1.5b-instruct-q4_k_m.gguf" })
        ).await;

        let llm = LlmClient::new(&manager).await.unwrap();
        let ctx = AgentContext::new(
            "t",
            "s",
            sandbox.db,
            llm,
            sandbox.domain_root.clone(),
            sandbox.domain_root.clone(),
        )
        .await;
        let agent = TransverseAgent::new();

        let intent = EngineeringIntent::VerifyQuality {
            scope: "code".into(),
            target: "Fantome".into(),
        };

        let result = agent.process(&ctx, &intent).await.unwrap().unwrap();
        assert!(result.message.contains("NON CONFORME"));
        assert!(result.message.contains("INCONNU dans le modèle"));
    }
}
