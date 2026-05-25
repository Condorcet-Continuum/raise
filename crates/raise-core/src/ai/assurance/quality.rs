// FICHIER : src-tauri/src/ai/assurance/quality.rs

use crate::utils::prelude::*;

/// 📈 CATÉGORIES DE MÉTRIQUES D'ASSURANCE
/// Définit l'axe d'évaluation conforme à la norme DO-178C / AI Act.
#[derive(Debug, Serializable, Deserializable, Clone, PartialEq)]
pub enum MetricCategory {
    Performance, // Précision, Rappel, F1-Score...
    Robustness,  // Stabilité face au bruit ou données hors-distribution
    Fairness,    // Détection de biais sémantiques
    Efficiency,  // Latence, consommation VRAM (Critique pour limite 8 Go)
}

/// 🚥 STATUT DE CONFORMITÉ
#[derive(Debug, Serializable, Deserializable, Clone, PartialEq)]
pub enum QualityStatus {
    Pass,    // Conforme aux exigences de sécurité
    Warning, // Dérive mineure détectée
    Fail,    // Échec critique - nécessite une intervention humaine (HITL)
}

#[derive(Debug, Serializable, Deserializable, Clone)]
pub struct QualityMetric {
    pub name: String,
    pub category: MetricCategory,
    pub value: f64,
    pub threshold_min: Option<f64>,
    pub threshold_max: Option<f64>,
    pub is_critical: bool,
    pub passed: bool,
}

/// 📄 RAPPORT DE QUALITÉ (Artefact de Gouvernance)
/// Cet objet est destiné à être hydraté en JSON-LD pour le Knowledge Graph.
#[derive(Debug, Serializable, Deserializable, Clone)]
pub struct QualityReport {
    #[serde(rename = "_id")]
    pub id: String,
    pub model_id: String,
    pub dataset_version: String,
    pub timestamp: i64,
    pub metrics: Vec<QualityMetric>,
    pub overall_status: QualityStatus,
    /// Score global (0.0 à 100.0)
    pub global_score: f64,
}

impl QualityReport {
    pub fn new(model_id: &str, dataset_version: &str) -> Self {
        Self {
            id: UniqueId::new_v4().to_string(),
            model_id: model_id.to_string(),
            dataset_version: dataset_version.to_string(),
            timestamp: UtcClock::now().timestamp(),
            metrics: Vec::new(),
            overall_status: QualityStatus::Warning,
            global_score: 0.0,
        }
    }

    /// Ajoute une métrique et déclenche la réévaluation automatique du statut.
    pub fn add_metric(
        &mut self,
        name: &str,
        category: MetricCategory,
        value: f64,
        min: Option<f64>,
        max: Option<f64>,
        is_critical: bool,
    ) {
        let mut passed = true;
        if let Some(min_val) = min {
            if value < min_val {
                passed = false;
            }
        }
        if let Some(max_val) = max {
            if value > max_val {
                passed = false;
            }
        }

        self.metrics.push(QualityMetric {
            name: name.to_string(),
            category,
            value,
            threshold_min: min,
            threshold_max: max,
            is_critical,
            passed,
        });

        self.evaluate_status();
    }

    /// Calcule l'état de santé global du modèle.
    /// 🎯 LOGIQUE : Un seul échec critique (is_critical: true) entraîne un statut FAIL.
    fn evaluate_status(&mut self) {
        if self.metrics.is_empty() {
            self.overall_status = QualityStatus::Warning;
            self.global_score = 0.0;
            return;
        }

        let has_critical_failure = self.metrics.iter().any(|m| m.is_critical && !m.passed);
        let has_minor_failure = self.metrics.iter().any(|m| !m.is_critical && !m.passed);

        self.overall_status = if has_critical_failure {
            QualityStatus::Fail
        } else if has_minor_failure {
            QualityStatus::Warning
        } else {
            QualityStatus::Pass
        };

        let passed_count = self.metrics.iter().filter(|m| m.passed).count();
        self.global_score = (passed_count as f64 / self.metrics.len() as f64) * 100.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[serial_test::serial]
    fn test_quality_scoring_integrity() {
        let mut report = QualityReport::new("test_model", "v1");

        // Cas 1 : Succès nominal
        report.add_metric(
            "Accuracy",
            MetricCategory::Performance,
            0.95,
            Some(0.9),
            None,
            true,
        );
        assert_eq!(report.overall_status, QualityStatus::Pass);
        assert_eq!(report.global_score, 100.0);

        // Cas 2 : Échec mineur (Warning)
        report.add_metric(
            "Latency",
            MetricCategory::Efficiency,
            120.0,
            None,
            Some(100.0),
            false,
        );
        assert_eq!(report.overall_status, QualityStatus::Warning);
        assert!((report.global_score - 50.0).abs() < f64::EPSILON);
    }
}
