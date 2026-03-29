use super::{AnalysisResult, Analyzer};
use crate::utils::prelude::*;

#[derive(Default)]
pub struct SemanticAnalyzer; // Renommé pour cohérence avec le fichier

impl SemanticAnalyzer {
    pub fn new() -> Self {
        Self
    }

    /// Détecte le type d'élément Arcadia et applique les règles de dépendance
    fn extract_dependencies(&self, element: &JsonValue, result: &mut AnalysisResult) {
        let element_type = element
            .get("@type")
            .or_else(|| element.get("type"))
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown");

        match element_type {
            "LogicalComponent" | "SystemComponent" | "PhysicalComponent" => {
                self.analyze_component(element, result);
            }
            "LogicalFunction" | "SystemFunction" | "PhysicalFunction" => {
                self.analyze_function(element, result);
            }
            _ => {
                self.analyze_generic_refs(element, result);
            }
        }
    }

    fn analyze_component(&self, component: &JsonValue, result: &mut AnalysisResult) {
        // 1. Dépendances fonctionnelles (Allocation)
        if let Some(allocations) = component
            .get("ownedFunctionalAllocation")
            .and_then(|v| v.as_array())
        {
            for alloc in allocations {
                if let Some(target_id) = alloc.get("target").and_then(|v| v.as_str()) {
                    // On utilise le handle sémantique strict "fn:ID"
                    result.dependencies.push(format!("fn:{}", target_id));
                }
            }
        }

        // 2. Dépendances structurelles (Sous-composants)
        if let Some(children) = component
            .get("ownedLogicalComponents")
            .and_then(|v| v.as_array())
        {
            for child in children {
                if let Some(child_name) = child.get("name").and_then(|v| v.as_str()) {
                    // Les sous-composants sont des dépendances directes
                    result.dependencies.push(format!("comp:{}", child_name));
                }
            }
        }
    }

    fn analyze_function(&self, function: &JsonValue, result: &mut AnalysisResult) {
        // Une fonction dépend de ses échanges (Inputs/Outputs)
        if let Some(inputs) = function
            .get("incomingFunctionalExchanges")
            .and_then(|v| v.as_array())
        {
            if !inputs.is_empty() {
                // Dépendance vers le handler sémantique système
                result.dependencies.push("sys:exchange_handler".to_string());
            }
        }
    }

    fn analyze_generic_refs(&self, element: &JsonValue, result: &mut AnalysisResult) {
        if let Some(obj) = element.as_object() {
            for (key, val) in obj {
                if key == "base_class" || key == "implements" {
                    if let Some(ref_name) = val.as_str() {
                        result.dependencies.push(format!("base:{}", ref_name));
                    }
                }
            }
        }
    }
}

impl Analyzer for SemanticAnalyzer {
    fn analyze(&self, model: &JsonValue) -> RaiseResult<AnalysisResult> {
        let mut result = AnalysisResult::default();
        self.extract_dependencies(model, &mut result);
        Ok(result)
    }
}

// =========================================================================
// TESTS UNITAIRES (Corrigés pour la V2)
// =========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_component_dependencies_extraction() {
        let analyzer = SemanticAnalyzer::new();

        let component = json_value!({
            "@type": "LogicalComponent",
            "name": "FlightManager",
            "ownedFunctionalAllocation": [
                { "target": "ComputeRouteID" }
            ],
            "ownedLogicalComponents": [
                { "name": "Autopilot" }
            ]
        });

        let result = analyzer.analyze(&component).expect("Analyse échouée");

        // On vérifie que les handles sont corrects dans 'dependencies'
        assert!(result
            .dependencies
            .contains(&"fn:ComputeRouteID".to_string()));
        assert!(result.dependencies.contains(&"comp:Autopilot".to_string()));
    }

    #[test]
    fn test_function_dependencies() {
        let analyzer = SemanticAnalyzer::new();
        let function = json_value!({
            "@type": "LogicalFunction",
            "name": "Calculate",
            "incomingFunctionalExchanges": ["Exchange_01"]
        });

        let result = analyzer.analyze(&function).expect("Analyse échouée");
        assert!(result
            .dependencies
            .contains(&"sys:exchange_handler".to_string()));
    }
}
