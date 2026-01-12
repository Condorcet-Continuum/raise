use super::{GeneratedFile, LanguageGenerator};
use crate::code_generator::templates::template_engine::TemplateEngine;
use anyhow::Result;
use heck::ToSnakeCase;
use serde_json::Value;
use std::path::PathBuf;
use tera::Context;

#[derive(Default)]
pub struct VerilogGenerator;

impl VerilogGenerator {
    pub fn new() -> Self {
        Self
    }
}

impl LanguageGenerator for VerilogGenerator {
    fn generate(
        &self,
        element: &Value,
        template_engine: &TemplateEngine,
    ) -> Result<Vec<GeneratedFile>> {
        let mut context = Context::new();

        let name = element
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown_module");
        let id = element.get("id").and_then(|v| v.as_str()).unwrap_or("0000");
        let desc = element
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("No description");

        context.insert("name", name);
        context.insert("id", id);
        context.insert("description", desc);

        // Rendu du template
        let content = template_engine.render("verilog/module", &context)?;

        // Nom de fichier en snake_case (ex: traffic_light.v)
        let filename = format!("{}.v", name.to_snake_case());

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
    fn test_verilog_gen() {
        let gen = VerilogGenerator::new();
        let engine = TemplateEngine::new();

        let element = json!({
            "name": "UartDriver",
            "id": "UART_01",
            "description": "Serial communication"
        });

        let files = gen.generate(&element, &engine).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path.to_str().unwrap(), "uart_driver.v");
        assert!(files[0].content.contains("module uart_driver"));
    }
}
