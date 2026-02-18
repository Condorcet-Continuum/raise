use crate::utils::{prelude::*, HashMap};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum ExplanationScope {
    Local,
    Global,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum XaiMethod {
    Shap { variant: String },
    Lime,
    AttentionMap,
    IntegratedGradients,
    GradCam,
    ChainOfThought,
    Counterfactual,
    Manual,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FeatureImportance {
    pub feature_id: String,
    pub raw_value: String,
    pub attribution_score: f32,
    pub rank: usize,
    pub confidence_interval: Option<(f32, f32)>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct VisualArtifact {
    pub artifact_type: String,
    pub mime_type: String,
    pub payload: String,
    pub description: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct XaiFrame {
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
    pub meta: HashMap<String, String>,
}

impl XaiFrame {
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

    pub fn add_feature(&mut self, id: &str, val: &str, score: f32, rank: usize) {
        self.features.push(FeatureImportance {
            feature_id: id.to_string(),
            raw_value: val.to_string(),
            attribution_score: score,
            rank,
            confidence_interval: None,
        });
    }

    pub fn add_visual(&mut self, type_: &str, mime: &str, data: &str) {
        self.visual_artifacts.push(VisualArtifact {
            artifact_type: type_.to_string(),
            mime_type: mime.to_string(),
            payload: data.to_string(),
            description: format!("Visualisation auto-generated via {:?}", self.method),
        });
    }

    /// Génère un résumé textuel pour inclusion dans un Prompt LLM (RAG)
    pub fn summarize_for_llm(&self) -> String {
        let mut summary = format!(
            "Explication ({:?}) pour la prédiction '{}'.\nFacteurs principaux :\n",
            self.method, self.predicted_output
        );

        // On prend le top 3 des features
        for f in self.features.iter().take(3) {
            summary.push_str(&format!(
                "- {} (Valeur: {}): Impact {:.2}\n",
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
    fn test_xai_summary() {
        let mut frame = XaiFrame::new("test", XaiMethod::Lime, ExplanationScope::Local);
        frame.predicted_output = "Rejected".into();
        frame.add_feature("Salary", "low", -0.8, 1);

        let summary = frame.summarize_for_llm();
        assert!(summary.contains("Rejected"));
        assert!(summary.contains("Salary"));
        assert!(summary.contains("-0.8"));
    }
}
