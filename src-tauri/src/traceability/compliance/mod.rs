pub mod do_178c;
pub mod eu_ai_act;
pub mod iec_61508;
pub mod iso_26262;

use crate::model_engine::types::ProjectModel;

pub trait ComplianceChecker {
    fn name(&self) -> &str;
    fn check(&self, model: &ProjectModel) -> ComplianceReport;
}

#[derive(Debug, serde::Serialize)]
pub struct ComplianceReport {
    pub standard: String,
    pub passed: bool,
    pub rules_checked: usize,
    pub violations: Vec<Violation>,
}

#[derive(Debug, serde::Serialize)]
pub struct Violation {
    pub element_id: Option<String>,
    pub rule_id: String,
    pub description: String,
    pub severity: String, // "Low", "Medium", "High", "Critical"
}
