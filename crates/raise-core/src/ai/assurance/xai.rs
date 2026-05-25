// FICHIER : src-tauri/src/ai/assurance/xai.rs

use crate::utils::prelude::*;

#[derive(Debug, Serializable, Deserializable, Clone, PartialEq)]
pub enum ExplanationScope {
    Local,  // Explication d'une instance précise (Inference)
    Global, // Explication du comportement global du modèle
}

/// Méthodes d'explicabilité supportées par le XaiFrame
#[derive(Debug, Serializable, Deserializable, Clone, PartialEq)]
pub enum XaiMethod {
    Shap { variant: String },
    Lime,
    AttentionMap,
    IntegratedGradients,
    GradCam,
    ChainOfThought, // Raisonnement textuel (LLM)
    Counterfactual, // Scénario "What-if"
    Manual,         // Annotation par un expert métier
}

#[derive(Debug, Serializable, Deserializable, Clone)]
pub struct FeatureImportance {
    pub feature_id: String,
    pub raw_value: String,
    pub attribution_score: f32,
    pub rank: usize,
    pub confidence_interval: Option<(f32, f32)>,
}

#[derive(Debug, Serializable, Deserializable, Clone)]
pub struct VisualArtifact {
    pub artifact_type: String, // ex: "heatmap", "tree"
    pub mime_type: String,
    pub payload: String, // Base64 ou URI
    pub description: String,
}

/// 🔮 XAI FRAME (Preuve d'Explicabilité)
/// Documente pourquoi une décision a été prise par un agent.
#[derive(Debug, Serializable, Deserializable, Clone)]
pub struct XaiFrame {
    #[serde(rename = "_id")]
    pub id: String,
    pub model_id: String,
    pub timestamp: i64,
    pub method: XaiMethod,
    pub scope: ExplanationScope,
    pub input_snapshot: String,
    pub predicted_output: String,
    pub features: Vec<FeatureImportance>,
    pub visual_artifacts: Vec<VisualArtifact>,
    pub fidelity_score: Option<f32>,
    pub computation_time_ms: u64,
    pub meta: UnorderedMap<String, String>,
}

impl XaiFrame {
    pub fn new(model_id: &str, method: XaiMethod, scope: ExplanationScope) -> Self {
        Self {
            id: UniqueId::new_v4().to_string(),
            model_id: model_id.to_string(),
            timestamp: UtcClock::now().timestamp(),
            method,
            scope,
            input_snapshot: String::new(),
            predicted_output: String::new(),
            features: Vec::new(),
            visual_artifacts: Vec::new(),
            fidelity_score: None,
            computation_time_ms: 0,
            meta: UnorderedMap::new(),
        }
    }

    pub fn add_feature(&mut self, id: &str, val: &str, score: f32, rank: usize) {
        self.features.push(FeatureImportance {
            feature_id: id.to_string(),
            raw_value: val.to_string(),
            attribution_score: score,
            rank,
            confidence_interval: None,
        });
    }

    /// Résumé structuré pour injection dans le contexte d'un Agent (Prompt Engineering)
    pub fn summarize_for_llm(&self) -> String {
        let mut summary = format!(
            "### Justification IA ({:?})\nSortie prédite : '{}'\nFacteurs d'influence prioritaires :\n",
            self.method, self.predicted_output
        );

        // Tri par importance absolue (rank)
        let mut sorted_features = self.features.clone();
        sorted_features.sort_by_key(|f| f.rank);

        for f in sorted_features.iter().take(5) {
            summary.push_str(&format!(
                "- **{}** (valeur: {}): Influence {:.2}\n",
                f.feature_id, f.raw_value, f.attribution_score
            ));
        }
        summary
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[serial_test::serial]
    fn test_xai_frame_summary_generation() {
        let mut frame = XaiFrame::new(
            "model_abc",
            XaiMethod::AttentionMap,
            ExplanationScope::Local,
        );
        frame.predicted_output = "Critical_Failure".into();
        frame.add_feature("Temperature", "150°C", 0.85, 1);

        let summary = frame.summarize_for_llm();
        assert!(summary.contains("Critical_Failure"));
        assert!(summary.contains("Temperature"));
        assert!(summary.contains("0.85"));
    }
}
