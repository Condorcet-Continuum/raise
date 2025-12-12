use serde::{Deserialize, Serialize};

/// Catégorie de la métrique mesurée
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum MetricCategory {
    Performance, // Précision, Rappel, F1-Score, RMSE...
    Robustness,  // Stabilité face au bruit, Attaques adverses
    Fairness,    // Biais démographique, Parité statistique
    Efficiency,  // Latence (ms), Consommation mémoire/CPU
}

/// Statut global du rapport de qualité
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum QualityStatus {
    Pass,    // Tous les seuils critiques sont respectés
    Warning, // Certains seuils secondaires sont dépassés
    Fail,    // Échec critique (modèle inutilisable en prod)
}

/// Une mesure unitaire (ex: "Accuracy = 0.95")
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct QualityMetric {
    pub name: String, // ex: "Accuracy"
    pub category: MetricCategory,
    pub value: f64,                 // La valeur mesurée
    pub threshold_min: Option<f64>, // Seuil minimal acceptable (si applicable)
    pub threshold_max: Option<f64>, // Seuil maximal acceptable (si applicable)
    pub is_critical: bool,          // Si true, un échec entraîne un Fail global
    pub passed: bool,               // Calculé automatiquement
}

/// Le Rapport de Qualité (Quality Report)
/// C'est l'artefact qui prouve que le modèle a été testé techniquement.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct QualityReport {
    pub id: String,
    pub model_id: String, // Lien vers le composant Architecture Physique (PA)
    pub dataset_version: String, // ID/Hash du jeu de données de test utilisé
    pub timestamp: i64,
    pub metrics: Vec<QualityMetric>,
    pub overall_status: QualityStatus,
}

impl QualityReport {
    /// Crée un nouveau rapport vide
    pub fn new(model_id: &str, dataset_version: &str) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            model_id: model_id.to_string(),
            dataset_version: dataset_version.to_string(),
            timestamp: chrono::Utc::now().timestamp(),
            metrics: Vec::new(),
            overall_status: QualityStatus::Warning, // Par défaut avant évaluation
        }
    }

    /// Ajoute une métrique et évalue immédiatement si elle passe le seuil
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

        // Recalcul du statut global
        self.evaluate_status();
    }

    /// Recalcule le statut global (Pass/Fail/Warning)
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
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quality_report_evaluation() {
        let mut report = QualityReport::new("model_v1", "dataset_2025");

        // 1. Ajout d'une métrique critique réussie (Accuracy > 0.90)
        report.add_metric(
            "Accuracy",
            MetricCategory::Performance,
            0.95,
            Some(0.90),
            None,
            true,
        );
        assert_eq!(report.overall_status, QualityStatus::Pass);

        // 2. Ajout d'une métrique mineure échouée (Latency < 50ms, réel 60ms)
        report.add_metric(
            "Latency",
            MetricCategory::Efficiency,
            60.0,
            None,
            Some(50.0),
            false, // Non critique
        );
        assert_eq!(report.overall_status, QualityStatus::Warning);

        // 3. Ajout d'une métrique critique échouée (Robustesse > 0.8)
        report.add_metric(
            "Robustness Score",
            MetricCategory::Robustness,
            0.5,
            Some(0.8),
            None,
            true, // Critique !
        );
        assert_eq!(report.overall_status, QualityStatus::Fail);
    }
}
