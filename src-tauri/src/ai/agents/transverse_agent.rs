// FICHIER : src-tauri/src/ai/agents/transverse_agent.rs

use crate::utils::{async_trait, data, prelude::*, Uuid};

use super::intent_classifier::EngineeringIntent;
use super::tools::{extract_json_from_llm, load_session, save_artifact, save_session};
use super::{Agent, AgentContext, AgentResult};
use crate::ai::llm::client::LlmBackend;
use crate::ai::nlp::entity_extractor;

use crate::json_db::collections::manager::CollectionsManager;
use crate::json_db::query::{Condition, FilterOperator, Query, QueryEngine, QueryFilter};

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
    ) -> Result<Value> {
        if original_name == "skip_llm" {
            return Ok(json!({}));
        }

        let response = ctx
            .llm
            .ask(LlmBackend::LocalLlama, sys, user)
            .await
            .map_err(|e| AppError::Validation(format!("LLM Transverse: {}", e)))?;

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
    ) -> Result<Value> {
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

    /// Recherche le composant dans la DB (SA, LA, ou PA)
    async fn find_component_in_db(&self, ctx: &AgentContext, name: &str) -> Option<Value> {
        // On suppose l'espace par défaut "mbse2/drones" (comme SoftwareAgent)
        // Dans une version future, cela devrait venir du contexte de session
        let manager = CollectionsManager::new(&ctx.db, "mbse2", "drones");
        let query_engine = QueryEngine::new(&manager);

        let collections = ["sa_components", "la_components", "pa_components"];

        for col in collections {
            let mut query = Query::new(col);
            query.filter = Some(QueryFilter {
                operator: FilterOperator::And,
                conditions: vec![Condition::eq("name", name.into())],
            });
            query.limit = Some(1);

            if let Ok(result) = query_engine.execute_query(query).await {
                if let Some(doc) = result.documents.first() {
                    return Some(doc.clone());
                }
            }
        }
        None
    }

    /// Analyse statique : DB + FS
    async fn perform_static_analysis(
        &self,
        ctx: &AgentContext,
        target: &str,
    ) -> (bool, Vec<String>) {
        let mut findings = Vec::new();
        let mut overall_success = true;

        // 1. VÉRIFICATION MODÈLE (DB)
        findings.push(format!(
            "🔎 Recherche de '{}' dans la base de données...",
            target
        ));

        let db_component = self.find_component_in_db(ctx, target).await;

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

                // Check Cargo.toml (Basic)
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
    ) -> Result<Option<AgentResult>> {
        let mut session = load_session(ctx)
            .await
            .unwrap_or_else(|_| super::AgentSession::new(&ctx.session_id, &ctx.agent_id));

        match intent {
            // CAS 1 : CRÉATION D'EXIGENCES
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

            // CAS 2 : VÉRIFICATION QUALITÉ
            EngineeringIntent::VerifyQuality { scope, target } => {
                session.add_message(
                    "user",
                    &format!("Verify Quality ({}) for {}", scope, target),
                );

                // Appel Async de l'analyse
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
    use crate::json_db::storage::{JsonDbConfig, StorageEngine};
    use crate::utils::{
        io::{self, tempdir},
        Arc,
    };

    #[test]
    fn test_transverse_id() {
        assert_eq!(TransverseAgent::new().id(), "quality_manager");
    }

    #[tokio::test]
    async fn test_verify_quality_logic() {
        let dir = tempdir().unwrap();
        let domain_root = dir.path().to_path_buf();
        let dataset_root = dir.path().join("dataset");

        let config = JsonDbConfig::new(domain_root.clone());
        let db = Arc::new(StorageEngine::new(config));
        let llm = LlmClient::new("http://localhost:11434", "dummy", None);

        // 1. Setup DB Manager
        let manager = CollectionsManager::new(&db, "mbse2", "drones");
        manager.init_db().await.unwrap();

        // 2. SEED DB : On crée le composant pour qu'il existe logiquement
        let comp_doc = json!({
            "id": "comp-123",
            "name": "Mon Composant",
            "layer": "LA",
            "type": "LogicalComponent"
        });
        manager
            .upsert_document("la_components", comp_doc)
            .await
            .unwrap();

        let ctx = AgentContext::new(
            "tester",
            "sess_qual_01",
            db,
            llm,
            domain_root.clone(),
            dataset_root,
        );

        let agent = TransverseAgent::new();

        // 3. Setup FS : Création physique du dossier et du Cargo.toml
        // 3. Setup FS : Création physique du dossier et du Cargo.toml
        let src_gen = domain_root.join("src-gen").join("mon_composant");
        let full_path = src_gen.join("Cargo.toml"); // On définit le fichier cible
        let content = "[package]\nname = \"mon_composant\"".to_string(); // On définit le contenu

        // ✅ Création du dossier parent
        io::create_dir_all(&src_gen)
            .await
            .expect("Échec de la création du dossier src-gen");

        // ✅ Écriture du fichier Cargo.toml
        io::write(&full_path, content)
            .await
            .expect("Échec de l'écriture du Cargo.toml"); // 4. Appel de l'intention
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
    async fn test_verify_quality_missing_in_db() {
        // Test cas d'échec : Composant absent de la DB
        let dir = tempdir().unwrap();
        let config = JsonDbConfig::new(dir.path().into());
        let db = Arc::new(StorageEngine::new(config));
        let llm = LlmClient::new("http://localhost:11434", "dummy", None);

        let ctx = AgentContext::new("t", "s", db, llm, dir.path().into(), dir.path().into());
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
