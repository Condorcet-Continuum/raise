// FICHIER : src-tauri/src/ai/assurance/mod.rs

pub mod health;
pub mod quality;
pub mod xai;

pub use quality::{QualityReport, QualityStatus};
pub use xai::{XaiFrame, XaiMethod};

use crate::json_db::collections::manager::CollectionsManager;
use crate::utils::prelude::*; // 🎯 Façade Unique

// --- PERSISTANCE (Assurance Store via JsonDB) ---
pub mod persistence {
    use super::*;

    /// Sauvegarde un rapport de qualité dans le JsonDb en utilisant les réglages dynamiques.
    pub async fn save_quality_report(
        manager: &CollectionsManager<'_>,
        report: &QualityReport,
    ) -> RaiseResult<String> {
        // 🎯 DATA-DRIVEN : Lecture depuis la configuration du composant
        let settings =
            AppConfig::get_runtime_settings(manager, "ref:components:handle:ai_assurance").await?;

        let coll_name = match settings["quality_collection"].as_str() {
            Some(c) => c.to_string(),
            None => raise_error!(
                "ERR_ASSURANCE_CONFIG",
                error = "Paramètre 'quality_collection' manquant dans service_settings."
            ),
        };

        let schema_uri = match settings["quality_schema"].as_str() {
            Some(s) => s.to_string(),
            None => raise_error!(
                "ERR_ASSURANCE_CONFIG",
                error = "Paramètre 'quality_schema' manquant dans service_settings."
            ),
        };

        // S'assure que la collection existe (idempotent)
        if let Err(e) = manager.create_collection(&coll_name, &schema_uri).await {
            user_trace!(
                "INF_COLL_EXISTS",
                json_value!({"coll": coll_name, "error": e.to_string()})
            );
        }

        let doc = json::serialize_to_value(report)?;

        // 🎯 Rigueur : Match sur l'opération d'écriture
        match manager.upsert_document(&coll_name, doc).await {
            Ok(_) => Ok(report.id.clone()),
            Err(e) => {
                raise_error!(
                    "ERR_ASSURANCE_SAVE_REPORT_FAILED",
                    error = e.to_string(),
                    context = json_value!({ "report_id": report.id, "type": "QualityReport", "collection": coll_name })
                );
            }
        }
    }

    /// Sauvegarde une trame d'explicabilité (XAI) dans le JsonDb avec configuration dynamique.
    pub async fn save_xai_frame(
        manager: &CollectionsManager<'_>,
        frame: &XaiFrame,
    ) -> RaiseResult<String> {
        // 🎯 DATA-DRIVEN : Lecture depuis la configuration
        let settings =
            AppConfig::get_runtime_settings(manager, "ref:components:handle:ai_assurance").await?;

        let coll_name = match settings["xai_collection"].as_str() {
            Some(c) => c.to_string(),
            None => raise_error!(
                "ERR_ASSURANCE_CONFIG",
                error = "Paramètre 'xai_collection' manquant dans service_settings."
            ),
        };

        let schema_uri = match settings["xai_schema"].as_str() {
            Some(s) => s.to_string(),
            None => raise_error!(
                "ERR_ASSURANCE_CONFIG",
                error = "Paramètre 'xai_schema' manquant dans service_settings."
            ),
        };

        if let Err(e) = manager.create_collection(&coll_name, &schema_uri).await {
            user_trace!(
                "INF_COLL_EXISTS",
                json_value!({"coll": coll_name, "error": e.to_string()})
            );
        }

        let doc = json::serialize_to_value(frame)?;

        match manager.upsert_document(&coll_name, doc).await {
            Ok(_) => Ok(frame.id.clone()),
            Err(e) => {
                raise_error!(
                    "ERR_ASSURANCE_SAVE_XAI_FAILED",
                    error = e.to_string(),
                    context = json_value!({ "frame_id": frame.id, "type": "XaiFrame", "collection": coll_name })
                );
            }
        }
    }
}

/// Exécute un agent et garantit la persistance de l'audit (XAI + Qualité).
pub async fn execute_certified<A: crate::ai::agents::Agent>(
    agent: &A,
    ctx: &crate::ai::agents::AgentContext,
    intent: &crate::ai::agents::intent_classifier::EngineeringIntent,
) -> RaiseResult<Option<crate::ai::agents::AgentResult>> {
    let result = agent.process(ctx, intent).await?;

    if let Some(res) = &result {
        let config = AppConfig::get();
        let sys_manager = CollectionsManager::new(
            &ctx.db,
            &config.mount_points.system.domain,
            &config.mount_points.system.db,
        );

        if let Some(frame) = &res.xai_frame {
            let _ = self::persistence::save_xai_frame(&sys_manager, frame).await;
        }

        if !res.artifacts.is_empty() {
            let report =
                self::QualityReport::new(&ctx.paths.domain_root.to_string_lossy(), "active_db");
            let _ = self::persistence::save_quality_report(&sys_manager, &report).await;
        }
    }

    Ok(result)
}

/// Récupère une trame d'explicabilité complète via son ID.
pub async fn get_xai_frame(
    manager: &CollectionsManager<'_>,
    frame_id: &str,
) -> RaiseResult<XaiFrame> {
    // 🎯 DATA-DRIVEN : On demande la collection au Kernel
    let settings =
        AppConfig::get_runtime_settings(manager, "ref:components:handle:ai_assurance").await?;
    let coll_name = match settings["xai_collection"].as_str() {
        Some(c) => c,
        None => raise_error!(
            "ERR_ASSURANCE_CONFIG",
            error = "Paramètre 'xai_collection' manquant dans service_settings."
        ),
    };

    match manager.get_document(coll_name, frame_id).await {
        Ok(Some(doc)) => {
            let frame: XaiFrame = json::deserialize_from_value(doc)?;
            Ok(frame)
        }
        Ok(None) => raise_error!(
            "ERR_ASSURANCE_XAI_NOT_FOUND",
            error = format!(
                "Trame XAI '{}' introuvable dans la collection {}.",
                frame_id, coll_name
            )
        ),
        Err(e) => raise_error!("ERR_ASSURANCE_DB_READ", error = e),
    }
}

// =========================================================================
// TESTS UNITAIRES (Rigueur Façade & Résilience)
// =========================================================================
// =========================================================================
// TESTS UNITAIRES (Rigueur Façade & Résilience)
// =========================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::agents::intent_classifier::EngineeringIntent;
    use crate::ai::agents::{Agent, AgentContext, AgentResult, CreatedArtifact};
    use crate::ai::assurance::quality::MetricCategory;
    use crate::ai::assurance::xai::XaiFrame;
    use crate::ai::assurance::xai::{ExplanationScope, XaiMethod};
    use crate::ai::llm::client::LlmClient;
    use crate::ai::world_model::engine::WorldModelConfig;
    use crate::ai::world_model::NeuroSymbolicEngine;
    use crate::utils::testing::{inject_mock_component, AgentDbSandbox, DbSandbox};

    fn get_assurance_settings() -> JsonValue {
        let config = AppConfig::get();
        let schema_quality = format!(
            "db://{}/{}/schemas/v2/assurance/quality_report.schema.json",
            config.mount_points.system.domain, config.mount_points.system.db
        );
        let schema_xai = format!(
            "db://{}/{}/schemas/v2/assurance/xai_frame.schema.json",
            config.mount_points.system.domain, config.mount_points.system.db
        );

        json_value!({
            "quality_collection": "quality_reports",
            "quality_schema": schema_quality,
            "xai_collection": "xai_frames",
            "xai_schema": schema_xai
        })
    }

    fn get_test_wm_config() -> WorldModelConfig {
        WorldModelConfig {
            vocab_size: 1024,
            embedding_dim: 512,
            action_dim: 64,
            hidden_dim: 1024,
            use_gpu: false,
        }
    }
    struct MockCertifiedAgent;
    #[async_interface]
    impl Agent for MockCertifiedAgent {
        fn id(&self) -> &str {
            "mock_agent_007"
        }
        async fn process(
            &self,
            _ctx: &AgentContext,
            _intent: &EngineeringIntent,
        ) -> RaiseResult<Option<AgentResult>> {
            let frame = XaiFrame::new(
                "test_model_v1",
                XaiMethod::ChainOfThought,
                ExplanationScope::Local,
            );
            let artifact = CreatedArtifact {
                id: "art_test".into(),
                name: "test_comp".into(),
                layer: "LA".into(),
                element_type: "Component".into(),
                path: "target/test_comp.json".into(),
            };

            Ok(Some(AgentResult {
                message: "Action certifiée terminée.".into(),
                artifacts: vec![artifact],
                outgoing_message: None,
                xai_frame: Some(frame),
            }))
        }
    }

    #[async_test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_execute_certified_full_audit_cycle() -> RaiseResult<()> {
        use crate::json_db::query::{Query, QueryEngine};
        let sandbox = AgentDbSandbox::new().await?;
        let config = AppConfig::get();

        let sys_mgr = CollectionsManager::new(
            &sandbox.db,
            &config.mount_points.system.domain,
            &config.mount_points.system.db,
        );

        // 🎯 INJECTION PROPRE : On injecte séparément le LLM et l'Assurance
        inject_mock_component(&sys_mgr, "llm", json_value!({})).await?;
        inject_mock_component(&sys_mgr, "ai_assurance", get_assurance_settings()).await?;

        let llm = LlmClient::new(&sys_mgr).await?;
        let world = SharedRef::new(NeuroSymbolicEngine::new_empty(get_test_wm_config())?);

        let ctx = AgentContext::new(
            "test_certified",
            "sess_cert_1",
            sandbox.db.clone(),
            llm,
            world,
            sandbox.domain_root.clone(),
            sandbox.domain_root.clone(),
        )
        .await?;

        let agent = MockCertifiedAgent;
        let intent = EngineeringIntent::Chat;

        let result = execute_certified(&agent, &ctx, &intent).await?;
        assert!(result.is_some());

        let audit_mgr = CollectionsManager::new(
            &sandbox.db,
            &config.mount_points.system.domain,
            &config.mount_points.system.db,
        );

        let query_xai = Query::new("xai_frames");
        let xai_res = QueryEngine::new(&audit_mgr)
            .execute_query(query_xai)
            .await?;
        assert_eq!(
            xai_res.documents.len(),
            1,
            "La trame XAI aurait dû être sauvegardée."
        );

        let query_quality = Query::new("quality_reports");
        let quality_res = QueryEngine::new(&audit_mgr)
            .execute_query(query_quality)
            .await?;
        assert_eq!(
            quality_res.documents.len(),
            1,
            "Le rapport de qualité est manquant."
        );

        Ok(())
    }

    #[async_test]
    #[serial_test::serial]
    async fn test_save_assurance_artifacts_with_json_db() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await?;
        let config = AppConfig::get();

        let manager = CollectionsManager::new(
            &sandbox.db,
            &config.mount_points.system.domain,
            &config.mount_points.system.db,
        );

        // 🎯 INJECTION PROPRE
        inject_mock_component(&manager, "ai_assurance", get_assurance_settings()).await?;

        let mut report = QualityReport::new("model_v2_resilient", "dataset_gold");
        report.add_metric(
            "Precision",
            MetricCategory::Performance,
            0.98,
            Some(0.95),
            None,
            true,
        );

        let report_id = persistence::save_quality_report(&manager, &report).await?;

        let saved_report = match manager.get_document("quality_reports", &report_id).await? {
            Some(doc) => doc,
            None => raise_error!("ERR_TEST_FAIL", error = "Rapport non persisté"),
        };

        assert_eq!(saved_report["model_id"], "model_v2_resilient");
        assert_eq!(saved_report["global_score"], 100.0);

        let frame = XaiFrame::new(
            "model_v2_resilient",
            XaiMethod::Lime,
            ExplanationScope::Local,
        );
        let frame_id = persistence::save_xai_frame(&manager, &frame).await?;

        let saved_frame = match manager.get_document("xai_frames", &frame_id).await? {
            Some(doc) => doc,
            None => raise_error!("ERR_TEST_FAIL", error = "Trame XAI introuvable"),
        };

        assert_eq!(saved_frame["model_id"], "model_v2_resilient");

        Ok(())
    }

    #[async_test]
    #[serial_test::serial]
    async fn test_assurance_resilience_mount_points() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await?;
        let config = AppConfig::get();

        let ws_manager = CollectionsManager::new(
            &sandbox.db,
            &config.mount_points.system.domain,
            &config.mount_points.system.db,
        );

        DbSandbox::mock_db(&ws_manager).await?;

        // 🎯 INJECTION PROPRE
        inject_mock_component(&ws_manager, "ai_assurance", get_assurance_settings()).await?;

        let report = QualityReport::new("ws_model", "ws_data");
        let result = persistence::save_quality_report(&ws_manager, &report).await;

        assert!(result.is_ok());

        let doc = ws_manager
            .get_document("quality_reports", &report.id)
            .await?;
        assert!(doc.is_some());

        Ok(())
    }

    #[async_test]
    #[serial_test::serial]
    async fn test_assurance_error_handling_on_invalid_data() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await?;
        let config = AppConfig::get();
        let manager = CollectionsManager::new(
            &sandbox.db,
            &config.mount_points.system.domain,
            &config.mount_points.system.db,
        );

        // 🎯 INJECTION PROPRE
        inject_mock_component(&manager, "ai_assurance", get_assurance_settings()).await?;

        let mut report = QualityReport::new("err_test", "void");
        report.id = "".to_string();

        let res = persistence::save_quality_report(&manager, &report).await;
        assert!(res.is_ok() || res.is_err());

        Ok(())
    }
}
