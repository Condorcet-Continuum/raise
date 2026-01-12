// FICHIER : src-tauri/src/model_engine/mod.rs

// 1. Modules Fondamentaux (Le cœur du moteur)
pub mod loader;
pub mod types;

// 2. Modules de Logique Métier (Les fonctionnalités)
pub mod arcadia; // Définitions sémantiques (OA, SA, LA, PA)
pub mod capella; // Support des fichiers .capella / .aird
pub mod transformers; // Génération de code et conversion
pub mod validators; // Vérification de cohérence

// 3. Re-exports (Façade publique pour le reste de l'app)

// Loader & Modèle
pub use loader::ModelLoader;
pub use types::ProjectModel;

// Transformers (Software, Hardware, System)
pub use transformers::{
    dialogue_to_model::DialogueToModelTransformer, get_transformer, ModelTransformer,
    TransformationDomain,
};

// Validators (Règles métier)
pub use validators::{
    consistency_checker::ConsistencyChecker, ModelValidator, Severity, ValidationIssue,
};

// Arcadia Semantics (Couches et Catégories)
pub use arcadia::{ArcadiaSemantics, ElementCategory, Layer};

// Capella (Import)
pub use capella::{CapellaReader, CapellaXmiParser};

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_integration_facade() {
        // 1. Vérifie l'accès aux types de base
        let _model = ProjectModel::default();

        // 2. Vérifie l'accès à la Factory Transformer
        let transformer = get_transformer(TransformationDomain::Software);
        let dummy = json!({ "id": "TEST", "name": "TestElement" });
        assert!(transformer.transform(&dummy).is_ok());

        // 3. Vérifie l'accès à l'enum Sémantique
        let layer = Layer::SystemAnalysis;
        assert_eq!(layer, Layer::SystemAnalysis);
    }
}
