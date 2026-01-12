pub mod change_tracker;
pub mod compliance;
pub mod impact_analyzer;
pub mod reporting;
pub mod tracer;

pub use change_tracker::ChangeTracker;
pub use impact_analyzer::ImpactAnalyzer;
pub use tracer::Tracer;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_exports() {
        let _ = ChangeTracker::new();
        // Vérifie que le compilateur lie bien les modules
        assert!(true);
    }
}
