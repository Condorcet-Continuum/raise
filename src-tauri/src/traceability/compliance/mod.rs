pub mod ai_governance;
pub mod do_178c;
pub mod eu_ai_act;
pub mod iec_61508;
pub mod iso_26262;

// Re-exports
pub use ai_governance::AiGovernanceChecker;
pub use do_178c::Do178cChecker;
pub use eu_ai_act::EuAiActChecker;
pub use iec_61508::Iec61508Checker;
pub use iso_26262::Iso26262Checker;

use crate::model_engine::types::ProjectModel;
use serde::{Deserialize, Serialize};

/// Interface que toute norme doit implémenter
pub trait ComplianceChecker {
    fn name(&self) -> &str;
    fn check(&self, model: &ProjectModel) -> ComplianceReport;
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ComplianceReport {
    pub standard: String,
    pub passed: bool,
    pub rules_checked: usize,
    pub violations: Vec<Violation>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Violation {
    pub element_id: Option<String>,
    pub rule_id: String,
    pub description: String,
    pub severity: String, // "Low", "Medium", "High", "Critical"
}

// =========================================================================
// TESTS UNITAIRES
// =========================================================================
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compliance_structures() {
        // Vérifie que les structures sont bien publiques et instanciables
        let violation = Violation {
            element_id: Some("id_123".into()),
            rule_id: "RULE-01".into(),
            description: "Test".into(),
            severity: "High".into(),
        };

        let report = ComplianceReport {
            standard: "TestStandard".into(),
            passed: false,
            rules_checked: 10,
            violations: vec![violation],
        };

        assert_eq!(report.standard, "TestStandard");
        assert!(!report.passed);
    }
}
