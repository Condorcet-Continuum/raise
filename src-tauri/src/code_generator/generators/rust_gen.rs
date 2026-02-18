// FICHIER : src-tauri/src/code_generator/generators/rust_gen.rs

use super::{GeneratedFile, LanguageGenerator};
use crate::code_generator::templates::template_engine::TemplateEngine;

use crate::utils::{
    data::{ContextBuilder, Deserialize, Value},
    io::PathBuf,
    prelude::*,
    sys,
};

use heck::{ToPascalCase, ToSnakeCase};

#[derive(Default)]
pub struct RustGenerator;

impl RustGenerator {
    pub fn new() -> Self {
        Self
    }

    /// Tente de formater le code Rust via l'outil standard `rustfmt`.
    fn format_code(&self, raw_code: &str) -> String {
        // ✅ On utilise notre façade système.
        // Elle s'occupe de lancer rustfmt, lui envoyer le code, et récupérer le résultat.
        match sys::pipe_through("rustfmt", raw_code) {
            Ok(formatted) => formatted,
            Err(e) => {
                // Si rustfmt n'est pas installé ou plante, on log et on renvoie le code brut.
                warn!("⚠️ Impossible de lancer rustfmt : {}", e);
                raw_code.to_string()
            }
        }
    }
}

// --- STRUCTURES INTERNES ---

#[derive(Deserialize, Debug, Default)]
struct ComponentImpl {
    technology: String,
    #[serde(rename = "artifactName")]
    artifact_name: Option<String>,
}

#[derive(Deserialize, Debug)]
struct ArcadiaComponent {
    #[serde(default = "default_id")]
    id: String,
    #[serde(default = "default_name")]
    name: String,
    description: Option<String>,
    implementation: Option<ComponentImpl>,
    #[serde(default)]
    #[serde(rename = "allocatedFunctions")]
    allocated_functions: Vec<Value>,
}

fn default_id() -> String {
    "0000".to_string()
}
fn default_name() -> String {
    "Unnamed".to_string()
}

// --- IMPLÉMENTATION ---

impl LanguageGenerator for RustGenerator {
    fn generate(
        &self,
        element: &Value,
        template_engine: &TemplateEngine,
    ) -> Result<Vec<GeneratedFile>> {
        let component: ArcadiaComponent = crate::utils::data::from_value(element.clone())
            .unwrap_or(ArcadiaComponent {
                id: element
                    .get("id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("0000")
                    .to_string(),
                name: element
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Unnamed")
                    .to_string(),
                description: element
                    .get("description")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
                implementation: None,
                allocated_functions: vec![],
            });

        // 2. Extraction des fonctions pour l'IA
        let functions: Vec<String> = component
            .allocated_functions
            .iter()
            .map(|f| {
                if let Some(s) = f.as_str() {
                    s.split(':').next_back().unwrap_or(s).to_string()
                } else {
                    f.get("name")
                        .and_then(|n| n.as_str())
                        .unwrap_or("UnknownFunction")
                        .to_string()
                }
            })
            .collect();

        // ✅ CONSTRUCTION DU CONTEXTE VIA ContextBuilder
        let context = ContextBuilder::new()
            .with_part("name", &component.name)
            .with_part("id", &component.id)
            .with_part(
                "description",
                &component.description.clone().unwrap_or_default(),
            )
            .with_part("functions", &functions)
            .build(); // Retourne un Value

        let mut files = Vec::new();

        // 3. Logique de Génération Conditionnelle
        if let Some(impl_specs) = &component.implementation {
            if impl_specs.technology == "Rust_Crate" {
                return self.generate_crate(
                    &component,
                    impl_specs,
                    &functions,
                    &context, // On passe le Value ici
                    template_engine,
                );
            }
        }

        // 4. Fallback : Génération Legacy
        let content = template_engine.render("rust/actor", &context)?;

        let formatted_content = self.format_code(&content);

        files.push(GeneratedFile {
            path: PathBuf::from(format!("{}.rs", component.name.to_pascal_case())),
            content: formatted_content,
        });

        Ok(files)
    }
}

impl RustGenerator {
    fn generate_crate(
        &self,
        comp: &ArcadiaComponent,
        impl_specs: &ComponentImpl,
        functions: &[String],
        context: &Value, // ✅ CHANGÉ : &Context -> &Value
        template_engine: &TemplateEngine,
    ) -> Result<Vec<GeneratedFile>> {
        let crate_name = impl_specs
            .artifact_name
            .clone()
            .unwrap_or_else(|| comp.name.to_snake_case());
        let mut files = Vec::new();

        // A. Cargo.toml
        let cargo_content = template_engine.render("rust/cargo", context).unwrap_or_else(|_| {
            format!(
                "[package]\nname = \"{}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[dependencies]\nanyhow = \"1.0\"\n", 
                crate_name
            )
        });

        files.push(GeneratedFile {
            path: PathBuf::from(format!("{}/Cargo.toml", crate_name)),
            content: cargo_content,
        });

        // B. lib.rs
        let lib_content = template_engine
            .render("rust/lib", context)
            .unwrap_or_else(|_| {
                let mut code = format!(
                    "//! {}\n//! Component ID: {}\n\n",
                    comp.description.as_deref().unwrap_or("No description"),
                    comp.id
                );

                for func in functions {
                    let fn_name = func.to_snake_case();
                    code.push_str(&format!("/// Implements system function: {}\n", func));
                    code.push_str(&format!("pub fn {}() {{\n", fn_name));
                    code.push_str(&format!("    // AI_INJECTION_POINT: {}\n", fn_name));
                    code.push_str("    todo!(\"Waiting for neuro-symbolic implementation\");\n");
                    code.push_str("    // END_AI_INJECTION_POINT\n");
                    code.push_str("}\n\n");
                }
                code
            });

        let formatted_lib = self.format_code(&lib_content);

        files.push(GeneratedFile {
            path: PathBuf::from(format!("{}/src/lib.rs", crate_name)),
            content: formatted_lib,
        });

        Ok(files)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::data::json;

    fn setup_engine() -> TemplateEngine {
        let mut engine = TemplateEngine::new();
        engine
            .add_raw_template("rust/actor", "struct {{ name }};")
            .unwrap();
        engine
    }

    #[test]
    fn test_legacy_generation() {
        let generator = RustGenerator::new();
        let engine = setup_engine();
        let element = json!({ "name": "LegacyComponent", "id": "123" });
        let files = generator.generate(&element, &engine).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path.to_str().unwrap(), "LegacyComponent.rs");
    }

    #[test]
    fn test_mbse_crate_generation_with_formatting() {
        let generator = RustGenerator::new();
        let engine = setup_engine();

        let element = json!({
            "name": "VisionSystem",
            "id": "UUID-VISION",
            "description": "Handles camera input",
            "implementation": {
                "technology": "Rust_Crate",
                "artifactName": "vision-sys"
            },
            "allocatedFunctions": ["ref:sa:name:Detect Objects"]
        });

        let files = generator.generate(&element, &engine).unwrap();
        assert_eq!(files.len(), 2);

        let lib = &files[1];
        let content = &lib.content;

        assert!(content.contains("pub fn detect_objects()"));
        assert!(content.contains("// AI_INJECTION_POINT: detect_objects"));
        assert!(!content.is_empty());
    }
}
