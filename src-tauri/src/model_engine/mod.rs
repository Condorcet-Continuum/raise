// FICHIER : src-tauri/src/model_engine/mod.rs

// 1. Modules Fondamentaux (Le cœur du moteur)
pub mod loader;
pub mod types;

// 2. Modules de Logique Métier (Les fonctionnalités)
pub mod arcadia; // Définitions sémantiques (OA, SA, LA, PA)
pub mod capella; // Support des fichiers .capella / .aird
pub mod sysml2;
pub mod transformers; // Génération de code et conversion
pub mod validators; // Vérification de cohérence

// 3. Re-exports (Façade publique pour le reste de l'app)

// Loader & Modèle
pub use loader::ModelLoader;
// MISE À JOUR : On expose tous les types nécessaires, y compris la nouvelle couche Transverse
pub use types::{ArcadiaElement, NameType, ProjectMeta, ProjectModel, TransverseModel};

// Transformers (Software, Hardware, System)
pub use transformers::{
    dialogue_to_model::DialogueToModelTransformer, get_transformer, ModelTransformer,
    TransformationDomain,
};

// Validators (Règles métier)
// MISE À JOUR : On expose les nouveaux validateurs (Compliance, Dynamic)
pub use validators::{
    compliance_validator::ComplianceValidator, consistency_checker::ConsistencyChecker,
    dynamic_validator::DynamicValidator, ModelValidator, Severity, ValidationIssue,
};

// Arcadia Semantics (Couches et Catégories)
pub use arcadia::element_kind::{ArcadiaSemantics, ElementCategory, Layer};

// Capella (Import)
pub use capella::{CapellaReader, CapellaXmiParser};

pub use sysml2::{Sysml2Parser, Sysml2ToArcadiaMapper};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::data::json;

    #[test]
    fn test_integration_facade() {
        // 1. Vérifie l'accès aux types de base
        let mut model = ProjectModel::default();

        // Vérification de l'accès à la couche Transverse via la façade
        let req = ArcadiaElement {
            id: "REQ-1".to_string(),
            name: NameType::String("Test".to_string()),
            kind: "Requirement".to_string(),
            ..Default::default()
        };
        model.transverse.requirements.push(req);
        assert_eq!(model.transverse.requirements.len(), 1);

        // 2. Vérifie l'accès à la Factory Transformer
        let transformer = get_transformer(TransformationDomain::Software);
        let dummy = json!({ "id": "TEST", "name": "TestElement" });
        assert!(transformer.transform(&dummy).is_ok());

        // 3. Vérifie l'accès à l'enum Sémantique
        let layer = Layer::SystemAnalysis;
        assert_eq!(layer, Layer::SystemAnalysis);
    }
}
