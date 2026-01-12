pub mod audit_report;
pub mod trace_matrix;

// Re-exports pour simplifier l'accès depuis l'extérieur
pub use audit_report::{AuditGenerator, AuditReport};
pub use trace_matrix::{MatrixGenerator, TraceabilityMatrix};

// =========================================================================
// TESTS UNITAIRES (Intégration du module)
// =========================================================================
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reporting_module_visibility() {
        // Vérifie que les types sont bien exportés et accessibles
        let _matrix_type = std::any::type_name::<TraceabilityMatrix>();
        let _report_type = std::any::type_name::<AuditReport>();

        assert!(_matrix_type.contains("TraceabilityMatrix"));
        assert!(_report_type.contains("AuditReport"));
    }
}
