use super::{GeneratedFile, LanguageGenerator};
use crate::code_generator::templates::template_engine::TemplateEngine;
use crate::utils::{data::Value, io::PathBuf, prelude::*};
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

        // 🎯 MIGRATION V1.3 : Création native, propre et sans Builder !
        // Si votre template_engine.render attend un serde_json::Value :
        let context = crate::utils::prelude::json!({
            "name": name,
            "id": id,
            "description": desc
        });

        /* 💡 NOTE IMPORTANTE :
        Si votre méthode `render` exige spécifiquement un objet `tera::Context`
        (et non un Value générique), remplacez le bloc ci-dessus par :

        let context = tera::Context::from_serialize(crate::utils::prelude::json!({
            "name": name,
            "id": id,
            "description": desc
        })).unwrap_or_default();
        */

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

    fn setup_engine() -> TemplateEngine {
        let mut engine = TemplateEngine::new();
        // On enregistre des templates simplifiés pour valider la logique du test
        engine
            .add_raw_template("cpp/header", "class {{ name }} {};")
            .unwrap();
        engine
            .add_raw_template("cpp/source", "#include \"{{ name }}.hpp\"")
            .unwrap();
        engine
    }

    #[test]
    fn test_cpp_generation_produces_two_files() {
        let gen = CppGenerator::new();
        let engine = setup_engine();

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
