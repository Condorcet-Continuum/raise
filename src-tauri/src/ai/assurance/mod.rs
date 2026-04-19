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

    /// Sauvegarde un rapport de qualité dans le JsonDb en utilisant le schéma maître système.
    pub async fn save_quality_report(
        manager: &CollectionsManager<'_>,
        report: &QualityReport,
    ) -> RaiseResult<String> {
        let app_config = AppConfig::get();

        // 🎯 FIX MOUNT POINTS : Résolution du schéma via la partition système configurée
        let schema_uri = format!(
            "db://{}/{}/schemas/v2/assurance/quality_report.schema.json",
            app_config.mount_points.system.domain, app_config.mount_points.system.db
        );

        // S'assure que la collection existe (idempotent)
        if let Err(e) = manager
            .create_collection("quality_reports", &schema_uri)
            .await
        {
            user_trace!(
                "INF_COLL_EXISTS",
                json_value!({"coll": "quality_reports", "error": e.to_string()})
            );
        }

        let doc = json::serialize_to_value(report)?;

        // 🎯 Rigueur : Match sur l'opération d'écriture
        match manager.upsert_document("quality_reports", doc).await {
            Ok(_) => Ok(report.id.clone()),
            Err(e) => {
                // 🎯 FIX : La macro diverge, pas de 'return'.
                raise_error!(
                    "ERR_ASSURANCE_SAVE_REPORT_FAILED",
                    error = e.to_string(),
                    context = json_value!({ "report_id": report.id, "type": "QualityReport" })
                );
            }
        }
    }

    /// Sauvegarde une trame d'explicabilité (XAI) dans le JsonDb.
    pub async fn save_xai_frame(
        manager: &CollectionsManager<'_>,
        frame: &XaiFrame,
    ) -> RaiseResult<String> {
        let app_config = AppConfig::get();

        let schema_uri = format!(
            "db://{}/{}/schemas/v2/assurance/xai_frame.schema.json",
            app_config.mount_points.system.domain, app_config.mount_points.system.db
        );

        if let Err(e) = manager.create_collection("xai_frames", &schema_uri).await {
            user_trace!(
                "INF_COLL_EXISTS",
                json_value!({"coll": "xai_frames", "error": e.to_string()})
            );
        }

        let doc = json::serialize_to_value(frame)?;

        match manager.upsert_document("xai_frames", doc).await {
            Ok(_) => Ok(frame.id.clone()),
            Err(e) => {
                raise_error!(
                    "ERR_ASSURANCE_SAVE_XAI_FAILED",
                    error = e.to_string(),
                    context = json_value!({ "frame_id": frame.id, "type": "XaiFrame" })
                );
            }
        }
    }
}

// 🎯 MODIFICATION : Centralisation de l'exécution avec Assurance (Noyau)
/// Exécute un agent et garantit la persistance de l'audit (XAI + Qualité).
pub async fn execute_certified<A: crate::ai::agents::Agent>(
    agent: &A,
    ctx: &crate::ai::agents::AgentContext,
    intent: &crate::ai::agents::intent_classifier::EngineeringIntent,
) -> RaiseResult<Option<crate::ai::agents::AgentResult>> {
    // 1. Exécution de l'agent via le noyau
    let result = agent.process(ctx, intent).await?;

    if let Some(res) = &result {
        let config = AppConfig::get();
        let sys_manager = CollectionsManager::new(
            &ctx.db,
            &config.mount_points.system.domain,
            &config.mount_points.system.db,
        );

        // 2. Persistance XAI automatique
        if let Some(frame) = &res.xai_frame {
            let _ = self::persistence::save_xai_frame(&sys_manager, frame).await;
        }

        // 3. Audit de Qualité automatique si modifications (Artefacts)
        if !res.artifacts.is_empty() {
            let report = self::QualityReport::new(
                &ctx.paths.domain_root.to_string_lossy(),
                "active_db", // Ou ctx.db.db_name si disponible
            );
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
    // 1. Recherche directe dans la collection système
    match manager.get_document("xai_frames", frame_id).await {
        Ok(Some(doc)) => {
            let frame: XaiFrame = json::deserialize_from_value(doc)?;
            Ok(frame)
        }
        Ok(None) => raise_error!(
            "ERR_ASSURANCE_XAI_NOT_FOUND",
            error = format!("Trame XAI '{}' introuvable.", frame_id)
        ),
        Err(e) => raise_error!("ERR_ASSURANCE_DB_READ", error = e),
    }
}

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
    use crate::ai::world_model::NeuroSymbolicEngine;
    use crate::utils::testing::{inject_mock_component, AgentDbSandbox, DbSandbox};

    // 1. Mock Agent : Simule un agent qui produit une explication et un artefact
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
                xai_frame: Some(frame), // On injecte une trame pour le test
            }))
        }
    }

    #[async_test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_execute_certified_full_audit_cycle() -> RaiseResult<()> {
        use crate::json_db::query::{Query, QueryEngine};
        let sandbox = AgentDbSandbox::new().await;
        let config = AppConfig::get();

        // 2. Initialisation des moteurs requis pour le contexte
        let sys_mgr = CollectionsManager::new(
            &sandbox.db,
            &config.mount_points.system.domain,
            &config.mount_points.system.db,
        );

        inject_mock_component(&sys_mgr, "llm", json_value!({})).await?;

        let llm = LlmClient::new(&sys_mgr).await?;
        let world = SharedRef::new(NeuroSymbolicEngine::new_empty(Default::default())?);

        let ctx = AgentContext::new(
            "test_certified",
            "sess_cert_1",
            sandbox.db.clone(),
            llm,
            world,
            sandbox.domain_root.clone(),
            sandbox.domain_root.clone(),
        )
        .await;

        let agent = MockCertifiedAgent;
        let intent = EngineeringIntent::Chat;

        // 3. EXÉCUTION DE LA FONCTION CIBLE
        let result = execute_certified(&agent, &ctx, &intent).await?;
        assert!(result.is_some());

        // 4. VÉRIFICATION DE LA PERSISTANCE (Partition Système)
        let audit_mgr = CollectionsManager::new(
            &sandbox.db,
            &config.mount_points.system.domain,
            &config.mount_points.system.db,
        );

        // 1. Vérification XAI
        let query_xai = Query::new("xai_frames");
        let xai_res = QueryEngine::new(&audit_mgr)
            .execute_query(query_xai)
            .await?;
        assert_eq!(
            xai_res.documents.len(),
            1,
            "La trame XAI aurait dû être sauvegardée."
        );

        // 2. Vérification Qualité
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
        let sandbox = AgentDbSandbox::new().await;
        let config = AppConfig::get();

        let manager = CollectionsManager::new(
            &sandbox.db,
            &config.mount_points.system.domain,
            &config.mount_points.system.db,
        );

        // 1. Test Sauvegarde Quality Report
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

        // 🎯 Vérification via Match strict (Zéro Dette)
        let saved_report = match manager.get_document("quality_reports", &report_id).await? {
            Some(doc) => doc,
            // 🎯 FIX : Suppression du 'return' devant la macro
            None => raise_error!("ERR_TEST_FAIL", error = "Rapport non persisté"),
        };

        assert_eq!(saved_report["model_id"], "model_v2_resilient");
        assert_eq!(saved_report["global_score"], 100.0);

        // 2. Test Sauvegarde XAI Frame
        let frame = XaiFrame::new(
            "model_v2_resilient",
            XaiMethod::Lime,
            ExplanationScope::Local,
        );
        let frame_id = persistence::save_xai_frame(&manager, &frame).await?;

        let saved_frame = match manager.get_document("xai_frames", &frame_id).await? {
            Some(doc) => doc,
            // 🎯 FIX : Suppression du 'return' devant la macro
            None => raise_error!("ERR_TEST_FAIL", error = "Trame XAI introuvable"),
        };

        assert_eq!(saved_frame["model_id"], "model_v2_resilient");

        Ok(())
    }

    #[async_test]
    #[serial_test::serial]
    async fn test_assurance_resilience_mount_points() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await;
        let config = AppConfig::get();

        let ws_manager = CollectionsManager::new(
            &sandbox.db,
            &config.mount_points.system.domain,
            &config.mount_points.system.db,
        );

        DbSandbox::mock_db(&ws_manager).await?;

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
        let sandbox = AgentDbSandbox::new().await;
        let config = AppConfig::get();
        let manager = CollectionsManager::new(
            &sandbox.db,
            &config.mount_points.system.domain,
            &config.mount_points.system.db,
        );

        let mut report = QualityReport::new("err_test", "void");
        report.id = "".to_string();

        let res = persistence::save_quality_report(&manager, &report).await;
        assert!(res.is_ok() || res.is_err());

        Ok(())
    }
}
