use super::{GeneratedFile, LanguageGenerator};
use crate::code_generator::templates::template_engine::TemplateEngine;
use anyhow::Result;
use serde_json::Value;
use std::path::PathBuf;
use tera::Context;

#[derive(Default)]
pub struct RustGenerator;

impl RustGenerator {
    pub fn new() -> Self {
        Self
    }
}

impl LanguageGenerator for RustGenerator {
    fn generate(
        &self,
        element: &Value,
        template_engine: &TemplateEngine,
    ) -> Result<Vec<GeneratedFile>> {
        let mut context = Context::new();

        // Extraction sécurisée des champs
        let name = element
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("UnknownElement");
        let id = element.get("id").and_then(|v| v.as_str()).unwrap_or("0000");
        let desc = element
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("No description.");

        context.insert("name", name);
        context.insert("id", id);
        context.insert("description", desc);

        // Utilisation du moteur centralisé pour le rendu
        // Note: Le filtre pascal_case est géré dans le template "rust/actor"
        let content = template_engine.render("rust/actor", &context)?;

        // Calcul du nom de fichier (PascalCase.rs)
        // On utilise la crate heck ici aussi si besoin pour le nom de fichier,
        // ou une méthode utilitaire simple.
        use heck::ToPascalCase;
        let filename = format!("{}.rs", name.to_pascal_case());

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
    fn test_rust_generation() {
        let generator = RustGenerator::new();
        let mut engine = TemplateEngine::new();

        // On force un template spécifique pour le test pour ne pas dépendre des defaults
        engine
            .add_raw_template("rust/actor", "struct {{ name | pascal_case }};")
            .unwrap();

        let element = json!({
            "name": "super_module",
            "id": "123",
            "description": "test module"
        });

        let files = generator.generate(&element, &engine).unwrap();

        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path.to_str().unwrap(), "SuperModule.rs");
        assert_eq!(files[0].content, "struct SuperModule;");
    }
}
