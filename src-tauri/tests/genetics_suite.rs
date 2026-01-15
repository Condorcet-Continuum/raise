// Déclaration des sous-modules situés dans le dossier /genetics_suite/
pub mod genetics_suite {
    pub mod full_flow_test;
    pub mod integration_test;
}

// Optionnel : un alias pour simplifier l'accès si besoin
#[cfg(test)]
mod tests {
    #[test]
    fn test_suite_ready() {
        assert!(true);
    }
}
