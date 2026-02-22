// FICHIER : src-tauri/src/traceability/reporting/mod.rs

pub mod audit_report;
pub mod trace_matrix;

// Re-exports pour simplifier l'accès depuis les agents ou l'interface
pub use audit_report::{AuditGenerator, AuditReport, ModelStats};
pub use trace_matrix::{MatrixGenerator, TraceabilityMatrix};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reporting_exports_integrity() {
        // Vérifie que les types sont accessibles et correctement nommés
        // (Évite les régressions lors de renommages de fichiers)
        let _test_stats = ModelStats {
            total_elements: 0,
            total_functions: 0,
            total_components: 0,
            total_requirements: 0,
            total_scenarios: 0,
            total_functional_chains: 0,
        };
        assert!(true);
    }
}
