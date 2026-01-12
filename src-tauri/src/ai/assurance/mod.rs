pub mod quality;
pub mod xai;

pub use quality::{QualityReport, QualityStatus};
pub use xai::{XaiFrame, XaiMethod};

use anyhow::{Context, Result};
use std::path::Path;

// --- PERSISTANCE (Assurance Store) ---
pub mod persistence {
    use super::*;
    use serde::Serialize;
    use std::fs;

    /// Sauvegarde un rapport de qualité dans le dossier du projet
    pub fn save_quality_report(domain_root: &Path, report: &QualityReport) -> Result<String> {
        // Structure : un2/transverse/collections/quality_reports/<ID>.json
        let relative_path = format!(
            "un2/transverse/collections/quality_reports/{}.json",
            report.id
        );
        let full_path = domain_root.join(&relative_path);

        save_json(&full_path, report)?;
        Ok(relative_path)
    }

    /// Sauvegarde une trame XAI dans le dossier du projet
    pub fn save_xai_frame(domain_root: &Path, frame: &XaiFrame) -> Result<String> {
        // Structure : un2/transverse/collections/xai_frames/<ID>.json
        let relative_path = format!("un2/transverse/collections/xai_frames/{}.json", frame.id);
        let full_path = domain_root.join(&relative_path);

        save_json(&full_path, frame)?;
        Ok(relative_path)
    }

    /// Helper interne pour l'écriture disque sécurisée
    fn save_json<T: Serialize>(path: &Path, data: &T) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Impossible de créer le dossier {:?}", parent))?;
        }
        let json = serde_json::to_string_pretty(data)?;
        fs::write(path, json).with_context(|| format!("Echec écriture fichier {:?}", path))?;
        Ok(())
    }
}

// --- TESTS UNITAIRES ---
#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::assurance::quality::MetricCategory;
    use crate::ai::assurance::xai::ExplanationScope;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_save_assurance_artifacts() {
        // 1. Setup environnement temporaire
        let dir = tempdir().unwrap();
        let root_path = dir.path();

        // 2. Test Sauvegarde Quality Report
        let mut report = QualityReport::new("model_test", "dataset_v1");
        report.add_metric(
            "Accuracy",
            MetricCategory::Performance,
            0.95,
            Some(0.9),
            None,
            true,
        );

        let path_rel_report = persistence::save_quality_report(root_path, &report)
            .expect("Sauvegarde QualityReport échouée");

        let full_path_report = root_path.join(path_rel_report);
        assert!(
            full_path_report.exists(),
            "Le fichier QualityReport n'a pas été créé"
        );

        // Vérification contenu
        let content_report = fs::read_to_string(full_path_report).unwrap();
        assert!(content_report.contains("Accuracy"));
        assert!(content_report.contains("0.95"));

        // 3. Test Sauvegarde XAI Frame
        let frame = XaiFrame::new("model_test", XaiMethod::Lime, ExplanationScope::Local);

        let path_rel_xai =
            persistence::save_xai_frame(root_path, &frame).expect("Sauvegarde XaiFrame échouée");

        let full_path_xai = root_path.join(path_rel_xai);
        assert!(
            full_path_xai.exists(),
            "Le fichier XaiFrame n'a pas été créé"
        );

        // Vérification contenu
        let content_xai = fs::read_to_string(full_path_xai).unwrap();
        assert!(content_xai.contains("Lime"));
        assert!(content_xai.contains("model_test"));
    }
}
