use super::{GeneratedFile, LanguageGenerator};
use crate::code_generator::templates::template_engine::TemplateEngine;
use crate::utils::data::{ContextBuilder, Value}; // ✅
use crate::utils::io::PathBuf;
use crate::utils::Result;
use heck::ToSnakeCase;

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
        let name = element
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown_module");
        let id = element.get("id").and_then(|v| v.as_str()).unwrap_or("0000");
        let desc = element
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("No description");

        let context = ContextBuilder::new()
            .with_part("name", &name)
            .with_part("id", &id)
            .with_part("description", &desc)
            .build();

        let content = template_engine.render("verilog/module", &context)?;
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
    use crate::utils::data::json;

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
