use super::{GeneratedFile, LanguageGenerator};
use crate::code_generator::templates::template_engine::TemplateEngine;
use crate::utils::data::{ContextBuilder, Value};
use crate::utils::io::PathBuf;
use crate::utils::Result;
use heck::ToPascalCase;

#[derive(Default)]
pub struct CppGenerator;

impl CppGenerator {
    pub fn new() -> Self {
        Self
    }
}

impl LanguageGenerator for CppGenerator {
    fn generate(
        &self,
        element: &Value,
        template_engine: &TemplateEngine,
    ) -> Result<Vec<GeneratedFile>> {
        let name = element
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown");
        let id = element.get("id").and_then(|v| v.as_str()).unwrap_or("0000");
        let desc = element
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        // ✅ ContextBuilder
        let context = ContextBuilder::new()
            .with_part("name", &name)
            .with_part("id", &id)
            .with_part("description", &desc)
            .build();

        // 1. Génération du Header (.hpp)
        let header_content = template_engine.render("cpp/header", &context)?;
        let header_file = GeneratedFile {
            path: PathBuf::from(format!("{}.hpp", name.to_pascal_case())),
            content: header_content,
        };

        // 2. Génération du Source (.cpp)
        let source_content = template_engine.render("cpp/source", &context)?;
        let source_file = GeneratedFile {
            path: PathBuf::from(format!("{}.cpp", name.to_pascal_case())),
            content: source_content,
        };

        Ok(vec![header_file, source_file])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::data::json;

    #[test]
    fn test_cpp_generation_produces_two_files() {
        let gen = CppGenerator::new();
        let engine = TemplateEngine::new();

        let element = json!({
            "name": "NavigationSystem",
            "id": "NAV_001"
        });

        let files = gen.generate(&element, &engine).unwrap();

        assert_eq!(files.len(), 2);
        assert_eq!(files[0].path.to_str().unwrap(), "NavigationSystem.hpp");
        assert!(files[0].content.contains("class NavigationSystem"));
        assert_eq!(files[1].path.to_str().unwrap(), "NavigationSystem.cpp");
        assert!(files[1]
            .content
            .contains("#include \"NavigationSystem.hpp\""));
    }
}
