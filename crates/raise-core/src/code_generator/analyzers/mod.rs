use crate::utils::prelude::*;

pub mod semantic_analyzer;

/// 📊 Résultat d'analyse simplifié pour alimenter le Weaver
#[derive(Debug, Default, Clone)]
pub struct AnalysisResult {
    /// Les handles sémantiques requis (ex: "fn:init")
    pub dependencies: Vec<String>,
    /// Métadonnées extraites du modèle
    pub metadata: UnorderedMap<String, String>,
}

/// 📝 Contrat mathématique pour les analyseurs de code.
pub trait Analyzer {
    fn analyze(&self, element: &JsonValue) -> RaiseResult<AnalysisResult>;
}
