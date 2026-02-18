use crate::utils::prelude::*;

/// Catégorie de la métrique mesurée
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum MetricCategory {
    Performance, // Précision, Rappel, F1-Score...
    Robustness,  // Stabilité face au bruit
    Fairness,    // Biais
    Efficiency,  // Latence, CPU
}

/// Statut global du rapport de qualité
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum QualityStatus {
    Pass,    // Succès total
    Warning, // Succès mitigé
    Fail,    // Échec critique
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct QualityMetric {
    pub name: String,
    pub category: MetricCategory,
    pub value: f64,
    pub threshold_min: Option<f64>,
    pub threshold_max: Option<f64>,
    pub is_critical: bool,
    pub passed: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct QualityReport {
    pub id: String,
    pub model_id: String,
    pub dataset_version: String,
    pub timestamp: i64,
    pub metrics: Vec<QualityMetric>,
    pub overall_status: QualityStatus,
    /// Score global calculé (0.0 à 100.0)
    pub global_score: f64,
}

impl QualityReport {
    pub fn new(model_id: &str, dataset_version: &str) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            model_id: model_id.to_string(),
            dataset_version: dataset_version.to_string(),
            timestamp: chrono::Utc::now().timestamp(),
            metrics: Vec::new(),
            overall_status: QualityStatus::Warning,
            global_score: 0.0,
        }
    }

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

    fn evaluate_status(&mut self) {
        let has_critical_failure = self.metrics.iter().any(|m| m.is_critical && !m.passed);
        let has_minor_failure = self.metrics.iter().any(|m| !m.is_critical && !m.passed);

        self.overall_status = if has_critical_failure {
            QualityStatus::Fail
        } else if has_minor_failure {
            QualityStatus::Warning
        } else {
            QualityStatus::Pass
        };

        // Calcul du score global simple (Ratio de succès pondéré par la criticité ?)
        // Ici simple ratio de succès pour l'exemple
        if self.metrics.is_empty() {
            self.global_score = 0.0;
        } else {
            let passed_count = self.metrics.iter().filter(|m| m.passed).count();
            self.global_score = (passed_count as f64 / self.metrics.len() as f64) * 100.0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quality_scoring() {
        let mut report = QualityReport::new("test_model", "v1");

        // 1. Succès Critique (1/1 -> 100%)
        report.add_metric(
            "Acc",
            MetricCategory::Performance,
            0.9,
            Some(0.8),
            None,
            true,
        );
        assert_eq!(report.overall_status, QualityStatus::Pass);
        assert_eq!(report.global_score, 100.0);

        // 2. Échec Mineur (1/2 -> 50%)
        report.add_metric(
            "Lat",
            MetricCategory::Efficiency,
            100.0,
            None,
            Some(50.0),
            false,
        );
        assert_eq!(report.overall_status, QualityStatus::Warning);
        assert_eq!(report.global_score, 50.0);
    }
}
