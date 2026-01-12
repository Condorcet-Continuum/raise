use super::{GeneratedFile, LanguageGenerator};
use crate::code_generator::templates::template_engine::TemplateEngine;
use anyhow::Result;
use heck::ToSnakeCase;
use serde_json::Value;
use std::path::PathBuf;
use tera::Context;

#[derive(Default)]
pub struct VhdlGenerator;

impl VhdlGenerator {
    pub fn new() -> Self {
        Self
    }
}

impl LanguageGenerator for VhdlGenerator {
    fn generate(
        &self,
        element: &Value,
        template_engine: &TemplateEngine,
    ) -> Result<Vec<GeneratedFile>> {
        let mut context = Context::new();

        let name = element
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown_entity");
        let id = element.get("id").and_then(|v| v.as_str()).unwrap_or("0000");
        let desc = element
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("No description");

        context.insert("name", name);
        context.insert("id", id);
        context.insert("description", desc);

        // Rendu du template
        let content = template_engine.render("vhdl/entity", &context)?;

        // Nom de fichier en snake_case (ex: alu_core.vhd)
        let filename = format!("{}.vhd", name.to_snake_case());

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
    fn test_vhdl_gen() {
        let gen = VhdlGenerator::new();
        let engine = TemplateEngine::new();

        let element = json!({
            "name": "DisplayController",
            "id": "DISP_01",
            "description": "LCD Control"
        });

        let files = gen.generate(&element, &engine).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path.to_str().unwrap(), "display_controller.vhd");
        assert!(files[0].content.contains("entity display_controller is"));
    }
}
