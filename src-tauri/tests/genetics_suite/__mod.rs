// Déclaration des modules de tests présents dans le dossier genetics_suite
pub mod full_flow_test;
pub mod integration_test;

/// Ce module regroupe les tests d'intégration du moteur génétique.
/// Il valide la chaîne : Données JSON -> Model Engine -> Bridge -> Genetic Engine.
#[cfg(test)]
mod tests {
    // Test de santé pour vérifier que la suite de tests est bien chargée
    #[test]
    fn test_suite_initialization() {
        assert!(true, "La suite de tests genetics_suite est correctement initialisée.");
    }
}