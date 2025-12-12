use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Portée de l'explication
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum ExplanationScope {
    Local,  // Explique une prédiction spécifique (ex: Pourquoi cette image est un chat ?)
    Global, // Explique le comportement général du modèle (ex: Quels mots sont importants en général ?)
}

/// Méthodes d'explicabilité supportées
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum XaiMethod {
    // Méthodes agnostiques (Tabulaire / Texte)
    Shap { variant: String }, // ex: "KernelShap", "TreeShap"
    Lime,

    // Méthodes spécifiques Deep Learning (Vision / NLP)
    AttentionMap,        // Transformers attention weights
    IntegratedGradients, // Attribution de pixels
    GradCam,             // Class Activation Mapping (CNN)

    // Méthodes génératives / Conversationnelles
    ChainOfThought, // Extraction des étapes de raisonnement du LLM
    Counterfactual, // "Si X avait été Y, la réponse aurait changé"

    Manual, // Documentation humaine
}

/// Représentation d'une feature influente avec contexte statistique
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FeatureImportance {
    pub feature_id: String, // Nom de la colonne, index du token, ou coordonnée pixel
    pub raw_value: String,  // La valeur originale (ex: "5000", "Le mot 'Banque'")
    pub attribution_score: f32, // Score d'impact (-1.0 à +1.0 ou infini)
    pub rank: usize,        // Rang d'importance (1 = le plus important)
    pub confidence_interval: Option<(f32, f32)>, // Pour les méthodes stochastiques comme SHAP
}

/// Conteneur pour les artéfacts visuels (pour affichage frontend)
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct VisualArtifact {
    pub artifact_type: String, // "heatmap", "saliency_map", "decision_tree_svg"
    pub mime_type: String,     // "image/png", "text/html", "application/json"
    pub payload: String,       // Base64 string ou chemin relatif de fichier
    pub description: String,
}

/// La "Trame XAI" enrichie : Preuve complète d'explicabilité
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct XaiFrame {
    // --- Identité ---
    pub id: String,       // UUID de la trame
    pub model_id: String, // ID du composant PA concerné
    pub timestamp: i64,   // Date de génération

    // --- Méthodologie ---
    pub method: XaiMethod,
    pub scope: ExplanationScope,

    // --- Contexte d'Inférence (Snapshot) ---
    /// Résumé ou Hash de l'entrée qui a généré cette explication
    pub input_snapshot: String,
    /// La prédiction brute du modèle (ex: "Classe A", "0.98")
    pub predicted_output: String,

    // --- Résultats d'Explication ---
    /// Liste triée des facteurs d'influence
    pub features: Vec<FeatureImportance>,

    /// Artéfacts visuels optionnels (ex: image avec surbrillance)
    pub visual_artifacts: Vec<VisualArtifact>,

    // --- Métriques de Qualité de l'Explication ---
    /// Score de fidélité (est-ce que l'explication approxime bien le modèle ?)
    pub fidelity_score: Option<f32>,
    /// Temps de calcul de l'explication en ms (pour monitorer l'overhead)
    pub computation_time_ms: u64,

    // --- Métadonnées flexibles ---
    pub meta: HashMap<String, String>,
}

impl XaiFrame {
    /// Constructeur standard
    pub fn new(model_id: &str, method: XaiMethod, scope: ExplanationScope) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            model_id: model_id.to_string(),
            timestamp: chrono::Utc::now().timestamp(),
            method,
            scope,
            input_snapshot: String::new(),
            predicted_output: String::new(),
            features: Vec::new(),
            visual_artifacts: Vec::new(),
            fidelity_score: None,
            computation_time_ms: 0,
            meta: HashMap::new(),
        }
    }

    /// Ajoute une feature importante (helper)
    pub fn add_feature(&mut self, id: &str, val: &str, score: f32, rank: usize) {
        self.features.push(FeatureImportance {
            feature_id: id.to_string(),
            raw_value: val.to_string(),
            attribution_score: score,
            rank,
            confidence_interval: None,
        });
    }

    /// Ajoute une image/visuel (helper)
    pub fn add_visual(&mut self, type_: &str, mime: &str, data: &str) {
        self.visual_artifacts.push(VisualArtifact {
            artifact_type: type_.to_string(),
            mime_type: mime.to_string(),
            payload: data.to_string(),
            description: format!("Visualisation auto-generated via {:?}", self.method),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rich_xai_frame_construction() {
        // Scénario : Analyse de risque de crédit (Tabulaire)
        let mut frame = XaiFrame::new(
            "credit_score_model_v2",
            XaiMethod::Shap {
                variant: "TreeShap".into(),
            },
            ExplanationScope::Local,
        );

        // Contexte
        frame.input_snapshot = "User: ID_452, Income: 30k, Debt: High".into();
        frame.predicted_output = "Refus (Score 0.2)".into();
        frame.computation_time_ms = 150;

        // Facteurs explicatifs
        frame.add_feature("Dette_Totale", "50000", -0.45, 1);
        frame.add_feature("Revenu_Mensuel", "2500", -0.20, 2);
        frame.add_feature("Historique_Defaut", "0", 0.10, 3); // Positif

        // Vérifications
        assert_eq!(frame.features.len(), 3);
        assert_eq!(frame.features[0].feature_id, "Dette_Totale");

        if let XaiMethod::Shap { variant } = frame.method {
            assert_eq!(variant, "TreeShap");
        } else {
            panic!("Mauvais type de méthode");
        }
    }

    #[test]
    fn test_visual_artifact() {
        // Scénario : Classification d'image (Vision)
        let mut frame = XaiFrame::new("resnet_50", XaiMethod::GradCam, ExplanationScope::Local);

        // Simulation d'une heatmap en base64
        frame.add_visual("heatmap", "image/png", "iVBORw0KGgoAAAANSUhEUgAAAAE...");

        assert_eq!(frame.visual_artifacts.len(), 1);
        assert_eq!(frame.visual_artifacts[0].mime_type, "image/png");
    }
}
