// FICHIER : src-tauri/src/ai/assurance/mod.rs

pub mod quality;
pub mod xai;

pub use quality::{QualityReport, QualityStatus};
pub use xai::{XaiFrame, XaiMethod};

use crate::json_db::collections::manager::CollectionsManager;
use crate::utils::prelude::*;

// --- PERSISTANCE (Assurance Store via JsonDB) ---
pub mod persistence {
    use super::*;

    /// Sauvegarde un rapport de qualité dans le JsonDb
    pub async fn save_quality_report(
        manager: &CollectionsManager<'_>, // 🎯 FIX: On injecte le manager directement !
        report: &QualityReport,
    ) -> RaiseResult<String> {
        // S'assure que la collection existe avant d'écrire
        let _ = manager.create_collection("quality_reports", None).await;

        let doc = crate::utils::data::to_value(report)?;

        // L'Upsert gère automatiquement l'indexation et la validation du schéma
        manager.upsert_document("quality_reports", doc).await?;

        Ok(report.id.clone())
    }

    /// Sauvegarde une trame XAI dans le JsonDb
    pub async fn save_xai_frame(
        manager: &CollectionsManager<'_>, // 🎯 FIX: Idem ici
        frame: &XaiFrame,
    ) -> RaiseResult<String> {
        let _ = manager.create_collection("xai_frames", None).await;

        let doc = crate::utils::data::to_value(frame)?;

        manager.upsert_document("xai_frames", doc).await?;

        Ok(frame.id.clone())
    }
}

// --- TESTS UNITAIRES ---
#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::assurance::quality::MetricCategory;
    use crate::ai::assurance::xai::ExplanationScope;
    use crate::utils::config::test_mocks::AgentDbSandbox;

    #[tokio::test]
    async fn test_save_assurance_artifacts_with_json_db() {
        let sandbox = AgentDbSandbox::new().await;
        let manager = CollectionsManager::new(
            &sandbox.db,
            &sandbox.config.system_domain,
            &sandbox.config.system_db,
        );
        // 3. Test Sauvegarde Quality Report
        let mut report = QualityReport::new("model_test", "dataset_v1");
        report.add_metric(
            "Accuracy",
            MetricCategory::Performance,
            0.95,
            Some(0.9),
            None,
            true,
        );

        let report_id = persistence::save_quality_report(&manager, &report)
            .await
            .expect("Sauvegarde QualityReport échouée");

        // 🎯 Vérification via le manager (Lecture DB au lieu d'un fichier direct)
        let saved_report = manager
            .get_document("quality_reports", &report_id)
            .await
            .unwrap()
            .expect("Document QualityReport introuvable en DB");

        assert_eq!(saved_report["model_id"], "model_test");
        assert_eq!(saved_report["global_score"], 100.0);

        // 4. Test Sauvegarde XAI Frame
        let frame = XaiFrame::new("model_test", XaiMethod::Lime, ExplanationScope::Local);

        let frame_id = persistence::save_xai_frame(&manager, &frame)
            .await
            .expect("Sauvegarde XaiFrame échouée");

        // 🎯 Vérification via le manager
        let saved_frame = manager
            .get_document("xai_frames", &frame_id)
            .await
            .unwrap()
            .expect("Document XAI introuvable en DB");

        assert_eq!(saved_frame["model_id"], "model_test");
    }
}
