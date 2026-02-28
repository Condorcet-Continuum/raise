// FICHIER : src-tauri/src/code_generator/generators/verilog_gen.rs

use super::{GeneratedFile, LanguageGenerator};
use crate::code_generator::templates::template_engine::TemplateEngine;
use crate::utils::{data::Value, io::PathBuf, prelude::*};
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
    ) -> RaiseResult<Vec<GeneratedFile>> {
        let name = element
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown_module");
        let id = element.get("id").and_then(|v| v.as_str()).unwrap_or("0000");
        let desc = element
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("No description");

        // 🎯 MIGRATION V1.3 : Création du contexte directe via json!
        let context = json!({
            "name": name,
            "id": id,
            "description": desc
        });

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

    // Configuration d'un moteur de template avec le fragment Verilog attendu
    fn setup_engine() -> TemplateEngine {
        let mut engine = TemplateEngine::new();
        engine
            .add_raw_template(
                "verilog/module",
                "module {{ name }} (/* {{ id }} */); endmodule",
            )
            .unwrap();
        engine
    }

    #[test]
    fn test_verilog_gen() {
        let gen = VerilogGenerator::new();
        let engine = setup_engine(); // ✅ Utilisation de l'engine configuré

        let element = json!({
            "name": "UartDriver",
            "id": "UART_01",
            "description": "Serial communication"
        });

        let files = gen.generate(&element, &engine).unwrap();

        assert_eq!(files.len(), 1);
        // Le générateur utilise .to_snake_case() pour le nom de fichier
        assert_eq!(files[0].path.to_str().unwrap(), "uart_driver.v");

        // L'assertion passera car le nom est injecté dans le template mocké
        assert!(files[0].content.contains("module UartDriver"));
    }
}
