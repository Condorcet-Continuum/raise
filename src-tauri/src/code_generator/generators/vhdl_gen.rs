use super::{GeneratedFile, LanguageGenerator};
use crate::code_generator::templates::template_engine::TemplateEngine;
use crate::utils::{
    data::{ContextBuilder, Value},
    io::PathBuf,
    prelude::*,
};
use heck::ToSnakeCase;

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
    ) -> RaiseResult<Vec<GeneratedFile>> {
        let name = element
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown_entity");
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

        let content = template_engine.render("vhdl/entity", &context)?;
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
    use crate::utils::data::json;

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
