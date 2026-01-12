use super::{GeneratedFile, LanguageGenerator};
use crate::code_generator::templates::template_engine::TemplateEngine;
use anyhow::Result;
use heck::ToPascalCase;
use serde_json::Value;
use std::path::PathBuf;
use tera::Context;

#[derive(Default)]
pub struct TypeScriptGenerator;

impl TypeScriptGenerator {
    pub fn new() -> Self {
        Self
    }
}

impl LanguageGenerator for TypeScriptGenerator {
    fn generate(
        &self,
        element: &Value,
        template_engine: &TemplateEngine,
    ) -> Result<Vec<GeneratedFile>> {
        let mut context = Context::new();

        let name = element
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown");
        let id = element.get("id").and_then(|v| v.as_str()).unwrap_or("0000");
        let desc = element
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        context.insert("name", name);
        context.insert("id", id);
        context.insert("description", desc);

        let content = template_engine.render("ts/class", &context)?;
        let filename = format!("{}.ts", name.to_pascal_case());

        Ok(vec![GeneratedFile {
            path: PathBuf::from(filename),
            content,
        }])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_ts_generation() {
        let gen = TypeScriptGenerator::new();
        let engine = TemplateEngine::new();

        let element = json!({
            "name": "UserInterface",
            "id": "UI_001",
            "description": "Main View"
        });

        let files = gen.generate(&element, &engine).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path.to_str().unwrap(), "UserInterface.ts");
        assert!(files[0].content.contains("export class UserInterface"));
    }
}
