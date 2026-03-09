use crate::code_generator::templates::template_engine::TemplateEngine;

use crate::utils::prelude::*;

// Liste complète des générateurs
pub mod cpp_gen; // NOUVEAU
pub mod rust_gen;
pub mod typescript_gen;
pub mod verilog_gen;
pub mod vhdl_gen; // NOUVEAU

#[derive(Debug, Clone, PartialEq)]
pub struct GeneratedFile {
    pub path: PathBuf,
    pub content: String,
}

pub trait LanguageGenerator: Send + Sync {
    fn generate(
        &self,
        element: &JsonValue,
        template_engine: &TemplateEngine,
    ) -> RaiseResult<Vec<GeneratedFile>>;
}

#[cfg(test)]
mod tests {
    // Les tests du trait ne changent pas
    use super::*;

    struct MockGenerator;
    impl LanguageGenerator for MockGenerator {
        fn generate(&self, _: &JsonValue, _: &TemplateEngine) -> RaiseResult<Vec<GeneratedFile>> {
            Ok(vec![])
        }
    }

    #[test]
    fn test_mock() {
        let gen = MockGenerator;
        let eng = TemplateEngine::new();
        assert!(gen.generate(&json_value!({}), &eng).is_ok());
    }
}
