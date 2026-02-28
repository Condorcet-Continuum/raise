// FICHIER : src-tauri/src/ai/assurance/mod.rs

pub mod quality;
pub mod xai;

pub use quality::{QualityReport, QualityStatus};
pub use xai::{XaiFrame, XaiMethod};

use crate::json_db::{
    collections::manager::CollectionsManager, // 🎯 Import du moteur de Base de Données
    storage::{JsonDbConfig, StorageEngine},
};
use crate::utils::{
    config::AppConfig, // 🎯 Import de la configuration
    prelude::*,
};
use std::path::Path;

// --- PERSISTANCE (Assurance Store via JsonDB) ---
pub mod persistence {
    use super::*;

    /// Sauvegarde un rapport de qualité dans le JsonDb (avec validation de schéma JSON-LD)
    pub async fn save_quality_report(
        domain_root: &Path,
        report: &QualityReport,
    ) -> RaiseResult<String> {
        let config = AppConfig::get();
        let domain = &config.system_domain;
        let db = &config.system_db;

        // 🎯 Initialisation du moteur JsonDB
        let db_config = JsonDbConfig::new(domain_root.to_path_buf());
        let storage = StorageEngine::new(db_config);
        let manager = CollectionsManager::new(&storage, domain, db);

        // S'assure que la collection existe avant d'écrire
        let _ = manager.create_collection("quality_reports", None).await;

        let doc = crate::utils::data::to_value(report)?;

        // 🎯 L'Upsert gère automatiquement l'indexation et la validation du schéma
        manager.upsert_document("quality_reports", doc).await?;

        Ok(report.id.clone())
    }

    /// Sauvegarde une trame XAI dans le JsonDb (avec validation de schéma JSON-LD)
    pub async fn save_xai_frame(domain_root: &Path, frame: &XaiFrame) -> RaiseResult<String> {
        let config = AppConfig::get();
        let domain = &config.system_domain;
        let db = &config.system_db;

        let db_config = JsonDbConfig::new(domain_root.to_path_buf());
        let storage = StorageEngine::new(db_config);
        let manager = CollectionsManager::new(&storage, domain, db);

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
    use crate::utils::config::test_mocks;
    use crate::utils::io::tempdir; // 🎯 Sandbox

    #[tokio::test]
    async fn test_save_assurance_artifacts_with_json_db() {
        // 🎯 Sandbox pour éviter de polluer le vrai répertoire
        test_mocks::inject_mock_config();
        let config = AppConfig::get();
        let domain = &config.system_domain;
        let db = &config.system_db;

        let dir = tempdir().unwrap();
        let root_path = dir.path();

        // Setup du manager pour vérifier la lecture à la fin du test
        let db_config = JsonDbConfig::new(root_path.to_path_buf());
        let storage = StorageEngine::new(db_config);
        let manager = CollectionsManager::new(&storage, domain, db);

        // 1. Test Sauvegarde Quality Report
        let mut report = QualityReport::new("model_test", "dataset_v1");
        report.add_metric(
            "Accuracy",
            MetricCategory::Performance,
            0.95,
            Some(0.9),
            None,
            true,
        );

        let report_id = persistence::save_quality_report(root_path, &report)
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

        // 2. Test Sauvegarde XAI Frame
        let frame = XaiFrame::new("model_test", XaiMethod::Lime, ExplanationScope::Local);

        let frame_id = persistence::save_xai_frame(root_path, &frame)
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
