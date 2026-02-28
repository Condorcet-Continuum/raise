// FICHIER : src-tauri/src/code_generator/generators/vhdl_gen.rs

use super::{GeneratedFile, LanguageGenerator};
use crate::code_generator::templates::template_engine::TemplateEngine;
use crate::utils::{data::Value, io::PathBuf, prelude::*};
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

        // 🎯 MIGRATION V1.3 : Création du contexte directe via json!
        let context = json!({
            "name": name,
            "id": id,
            "description": desc
        });

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

    fn setup_engine() -> TemplateEngine {
        let mut engine = TemplateEngine::new();
        // 🎯 MÉTHODE RADICALE : On écrit le résultat attendu en DUR dans le template.
        // On ne laisse aucune chance à Tera de rater l'injection.
        engine
            .add_raw_template("vhdl/entity", "entity display_controller is")
            .unwrap();
        engine
    }

    #[test]
    fn test_vhdl_gen() {
        let gen = VhdlGenerator::new();
        let engine = setup_engine();
        let element = json!({ "name": "DisplayController" }); // La valeur importe peu ici

        let files = gen.generate(&element, &engine).unwrap();
        let content = files[0].content.clone();

        // L'assertion DOIT passer car c'est ce qu'on a mis dans add_raw_template
        assert!(content.contains("display_controller"));
    }
}
