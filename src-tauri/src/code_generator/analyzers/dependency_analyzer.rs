use super::{AnalysisResult, Analyzer};
use crate::utils::data::Value;
use crate::utils::Result;

#[derive(Default)]
pub struct DependencyAnalyzer;

impl DependencyAnalyzer {
    pub fn new() -> Self {
        Self
    }

    /// Détecte le type d'élément Arcadia et applique les règles de dépendance
    fn extract_dependencies(&self, element: &Value, result: &mut AnalysisResult) {
        // Tentative de détection du type via JSON-LD (@type) ou structure simple (type)
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
                // Fallback : recherche générique de références
                self.analyze_generic_refs(element, result);
            }
        }
    }

    fn analyze_component(&self, component: &Value, result: &mut AnalysisResult) {
        // 1. Dépendances fonctionnelles (Allocation)
        // Un composant doit importer les fonctions qu'il héberge/exécute
        if let Some(allocations) = component
            .get("ownedFunctionalAllocation")
            .and_then(|v| v.as_array())
        {
            for alloc in allocations {
                if let Some(target_id) = alloc.get("target").and_then(|v| v.as_str()) {
                    // Simulation d'un namespace basé sur l'ID (à adapter selon votre structure réelle)
                    result
                        .imports
                        .insert(format!("crate::functions::Function_{}", target_id));
                }
            }
        }

        // 2. Dépendances structurelles (Enfants)
        if let Some(children) = component
            .get("ownedLogicalComponents")
            .and_then(|v| v.as_array())
        {
            for child in children {
                if let Some(child_name) = child.get("name").and_then(|v| v.as_str()) {
                    result.hard_dependencies.push(child_name.to_string());
                    // On importe aussi le type de l'enfant
                    result
                        .imports
                        .insert(format!("crate::components::{}", child_name));
                }
            }
        }
    }

    fn analyze_function(&self, function: &Value, result: &mut AnalysisResult) {
        // Une fonction dépend de ses échanges (Inputs/Outputs)
        if let Some(inputs) = function
            .get("incomingFunctionalExchanges")
            .and_then(|v| v.as_array())
        {
            if !inputs.is_empty() {
                result
                    .imports
                    .insert("crate::common::ExchangeHandler".to_string());
            }
        }
    }

    fn analyze_generic_refs(&self, element: &Value, result: &mut AnalysisResult) {
        // Scan récursif simple pour trouver des champs "type_ref" ou "base_class"
        if let Some(obj) = element.as_object() {
            for (key, val) in obj {
                if key == "base_class" || key == "implements" {
                    if let Some(ref_name) = val.as_str() {
                        result.imports.insert(format!("crate::base::{}", ref_name));
                    }
                }
            }
        }
    }
}

impl Analyzer for DependencyAnalyzer {
    fn analyze(&self, model: &Value) -> Result<AnalysisResult> {
        let mut result = AnalysisResult::default();
        self.extract_dependencies(model, &mut result);
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::data::json;

    #[test]
    fn test_component_dependencies_extraction() {
        let analyzer = DependencyAnalyzer::new();

        let component = json!({
            "@type": "LogicalComponent",
            "name": "FlightManager",
            // Allocation de fonction
            "ownedFunctionalAllocation": [
                { "target": "ComputeRouteID" }
            ],
            // Sous-composants
            "ownedLogicalComponents": [
                { "name": "Autopilot" },
                { "name": "GPS" }
            ]
        });

        let result = analyzer.analyze(&component).expect("Analyse échouée");

        // Vérification des imports
        assert!(result
            .imports
            .contains("crate::functions::Function_ComputeRouteID"));
        assert!(result.imports.contains("crate::components::Autopilot"));

        // Vérification des dépendances fortes
        assert_eq!(result.hard_dependencies.len(), 2);
        assert!(result.hard_dependencies.contains(&"Autopilot".to_string()));
    }

    #[test]
    fn test_function_dependencies() {
        let analyzer = DependencyAnalyzer::new();
        let function = json!({
            "@type": "LogicalFunction",
            "name": "Calculate",
            "incomingFunctionalExchanges": ["Exchange_01"]
        });

        let result = analyzer.analyze(&function).expect("Analyse échouée");
        assert!(result.imports.contains("crate::common::ExchangeHandler"));
    }
}
