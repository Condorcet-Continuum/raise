// FICHIER : src-tauri/src/code_generator/analyzers/semantic_analyzer.rs

use super::{AnalysisResult, Analyzer};
use crate::utils::prelude::*;

// =========================================================================
// 1. ONTOLOGIE STRICTE (Remplacement des Magic Strings)
// =========================================================================

#[derive(Debug, PartialEq, Eq)]
pub enum ArcadiaLayer {
    System,
    Logical,
    Physical,
}

#[derive(Debug, PartialEq, Eq)]
pub enum ArcadiaElementType {
    Component(ArcadiaLayer),
    Function(ArcadiaLayer),
}

impl Parsable for ArcadiaElementType {
    type Err = String;
    /// Parse le type brut issu du JSON-LD pour garantir la conformité au méta-modèle
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "SystemComponent" => Ok(Self::Component(ArcadiaLayer::System)),
            "LogicalComponent" => Ok(Self::Component(ArcadiaLayer::Logical)),
            "PhysicalComponent" => Ok(Self::Component(ArcadiaLayer::Physical)),
            "SystemFunction" => Ok(Self::Function(ArcadiaLayer::System)),
            "LogicalFunction" => Ok(Self::Function(ArcadiaLayer::Logical)),
            "PhysicalFunction" => Ok(Self::Function(ArcadiaLayer::Physical)),
            _ => Err(format!("Type Arcadia inconnu : {}", s)),
        }
    }
}

// =========================================================================
// 2. ANALYSEUR SÉMANTIQUE
// =========================================================================

#[derive(Default)]
pub struct SemanticAnalyzer;

impl SemanticAnalyzer {
    pub fn new() -> Self {
        Self
    }

    /// 🧠 Extrait un identifiant unique robuste (URN) pour le graphe topologique
    fn extract_urn(element: &JsonValue, prefix: &str) -> Option<String> {
        // 1. On privilégie l'ID unique (UUID) s'il existe dans l'export MBSE
        if let Some(id) = element.get("id").and_then(|v| v.as_str()) {
            return Some(format!("{}:{}", prefix, id));
        }
        // 2. Fallback déterministe sur le nom (ex: "Flight Manager" -> "flight_manager")
        if let Some(name) = element.get("name").and_then(|v| v.as_str()) {
            let normalized = name.replace(" ", "_").to_lowercase();
            return Some(format!("{}:{}", prefix, normalized));
        }
        None
    }

    /// Détecte le type d'élément Arcadia et dispatche l'analyse
    fn extract_dependencies(
        &self,
        element: &JsonValue,
        result: &mut AnalysisResult,
    ) -> RaiseResult<()> {
        let raw_type = match element
            .get("@type")
            .or_else(|| element.get("type"))
            .and_then(|v| v.as_str())
        {
            Some(t) => t,
            None => raise_error!(
                "ERR_MBSE_MISSING_TYPE",
                context = json_value!({
                    "hint": "L'élément JSON-LD doit posséder une propriété '@type' ou 'type' issue de l'ontologie Arcadia."
                })
            ),
        };

        match raw_type.parse::<ArcadiaElementType>() {
            Ok(ArcadiaElementType::Component(layer)) => {
                self.analyze_component(element, result, &layer)
            }
            Ok(ArcadiaElementType::Function(_layer)) => self.analyze_function(element, result),
            Err(_) => {
                // Fallback pour les références génériques (classes de base, traits)
                self.analyze_generic_refs(element, result)
            }
        }

        Ok(())
    }

    fn analyze_component(
        &self,
        component: &JsonValue,
        result: &mut AnalysisResult,
        layer: &ArcadiaLayer,
    ) {
        let layer_prefix = match layer {
            ArcadiaLayer::System => "sys",
            ArcadiaLayer::Logical => "log",
            ArcadiaLayer::Physical => "phy",
        };

        // 1. Dépendances fonctionnelles (Allocation)
        // Arcadia utilise des clés différentes selon la couche
        let alloc_key = match layer {
            ArcadiaLayer::System => "ownedSystemFunctionAllocations",
            ArcadiaLayer::Logical => "ownedFunctionalAllocation",
            ArcadiaLayer::Physical => "ownedPhysicalFunctionAllocations",
        };

        if let Some(allocations) = component.get(alloc_key).and_then(|v| v.as_array()) {
            for alloc in allocations {
                // On utilise le parseur d'URN strict
                if let Some(target_urn) = Self::extract_urn(alloc, "fn") {
                    result.dependencies.push(target_urn);
                } else if let Some(target_id) = alloc.get("target").and_then(|v| v.as_str()) {
                    // Fallback de compatibilité pour les vieux modèles
                    result.dependencies.push(format!("fn:{}", target_id));
                }
            }
        }

        // 2. Dépendances structurelles (Sous-composants)
        let subcomp_key = match layer {
            ArcadiaLayer::System => "ownedSystemComponents",
            ArcadiaLayer::Logical => "ownedLogicalComponents",
            ArcadiaLayer::Physical => "ownedPhysicalComponents",
        };

        if let Some(children) = component.get(subcomp_key).and_then(|v| v.as_array()) {
            for child in children {
                // Le prefix comp_log ou comp_phy évite les collisions inter-couches
                if let Some(child_urn) = Self::extract_urn(child, &format!("comp_{}", layer_prefix))
                {
                    result.dependencies.push(child_urn);
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
                // Dépendance vers le handler sémantique
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

        // Propagation stricte de l'erreur via match (selon tes directives, pas de `?` caché ou map_err)
        match self.extract_dependencies(model, &mut result) {
            Ok(_) => Ok(result),
            Err(e) => Err(e), // L'erreur est déjà une AppError structurée générée par raise_error!
        }
    }
}

// =========================================================================
// TESTS UNITAIRES (Refactorisés pour les URN stricts)
// =========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_component_dependencies_extraction_with_urn() {
        let analyzer = SemanticAnalyzer::new();

        // Ajout des ID pour vérifier la priorité du parseur d'URN
        let component = json_value!({
            "@type": "LogicalComponent",
            "name": "Flight Manager",
            "ownedFunctionalAllocation": [
                { "id": "uuid-func-123", "target": "ComputeRoute" }
            ],
            "ownedLogicalComponents": [
                { "name": "Auto Pilot" } // Sans ID, doit générer un fallback normalisé
            ]
        });

        let result = analyzer.analyze(&component).expect("Analyse échouée");

        // 1. Vérification de la priorité à l'UUID pour la fonction
        assert!(result
            .dependencies
            .contains(&"fn:uuid-func-123".to_string()));

        // 2. Vérification de la normalisation du fallback (espaces -> underscores) + prefixage de couche
        assert!(result
            .dependencies
            .contains(&"comp_log:auto_pilot".to_string()));
    }

    #[test]
    fn test_missing_type_triggers_raise_error() {
        let analyzer = SemanticAnalyzer::new();
        let invalid_element = json_value!({
            "name": "GhostComponent"
            // Pas de @type ou type
        });

        let result = analyzer.analyze(&invalid_element);

        assert!(result.is_err());
        let err = result.unwrap_err();
        let AppError::Structured(data) = err; // On déstructure ton variant
        assert_eq!(data.code, "ERR_MBSE_MISSING_TYPE");
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
