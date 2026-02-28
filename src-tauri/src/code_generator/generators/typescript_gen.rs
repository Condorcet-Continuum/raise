// FICHIER : src-tauri/src/code_generator/generators/typescript_gen.rs

use super::{GeneratedFile, LanguageGenerator};
use crate::code_generator::templates::template_engine::TemplateEngine;
use crate::utils::{
    data::Value, // 🗑️ Suppression de ContextBuilder
    io::PathBuf,
    prelude::*, // 🎯 Importe nativement json! et RaiseResult
};
use heck::ToPascalCase;

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
    ) -> RaiseResult<Vec<GeneratedFile>> {
        let name = element
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown");
        let id = element.get("id").and_then(|v| v.as_str()).unwrap_or("0000");
        let desc = element
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        // 🎯 MIGRATION V1.3 : Création du contexte directe et lisible via json!
        let context = json!({
            "name": name,
            "id": id,
            "description": desc
        });

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

    // 1. Fonction de configuration pour injecter un template de test
    fn setup_engine() -> TemplateEngine {
        let mut engine = TemplateEngine::new();
        // On simule le template attendu par le générateur
        engine
            .add_raw_template(
                "ts/class",
                "export class {{ name }} { /* {{ description }} */ }",
            )
            .unwrap();
        engine
    }

    #[test]
    fn test_ts_generation() {
        let gen = TypeScriptGenerator::new();
        let engine = setup_engine(); // ✅ On utilise l'engine configuré

        let element = json!({
            "name": "UserInterface",
            "id": "UI_001",
            "description": "Main View"
        });

        let files = gen.generate(&element, &engine).unwrap();

        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path.to_str().unwrap(), "UserInterface.ts");

        // ✅ Maintenant cette assertion passera car le template a été rendu
        assert!(files[0].content.contains("export class UserInterface"));
    }
}
