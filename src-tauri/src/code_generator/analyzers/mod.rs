pub mod dependency_analyzer;
pub mod injection_analyzer;

use anyhow::Result;
use serde_json::Value;
use std::collections::HashSet;

/// Résultat de l'analyse de dépendance pour un élément
#[derive(Debug, Default, Clone)]
pub struct AnalysisResult {
    /// Liste des modules/fichiers que cet élément doit importer (ex: "crate::models::Engine")
    pub imports: HashSet<String>,

    /// Liste des dépendances fortes qui nécessitent une définition préalable
    pub hard_dependencies: Vec<String>,
}

/// Trait que tout analyseur de modèle doit respecter
pub trait Analyzer {
    fn analyze(&self, model: &Value) -> Result<AnalysisResult>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_analysis_result_defaults() {
        let res = AnalysisResult::default();
        assert!(res.imports.is_empty());
        assert!(res.hard_dependencies.is_empty());
    }
}
