// FICHIER : src-tauri/src/code_generator/generators/rust_gen.rs

use super::{GeneratedFile, LanguageGenerator};
use crate::code_generator::templates::template_engine::TemplateEngine;
use anyhow::Result;
use heck::{ToPascalCase, ToSnakeCase};
use serde::Deserialize;
use serde_json::Value;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use tera::Context;

#[derive(Default)]
pub struct RustGenerator;

impl RustGenerator {
    pub fn new() -> Self {
        Self
    }

    /// Tente de formater le code Rust via l'outil standard `rustfmt`.
    /// Si l'outil n'est pas présent ou échoue, retourne le code brut.
    fn format_code(&self, raw_code: &str) -> String {
        let mut child = match Command::new("rustfmt")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null()) // On ignore les erreurs de stderr pour ne pas polluer les logs
            .spawn()
        {
            Ok(c) => c,
            Err(_) => return raw_code.to_string(), // rustfmt non installé, on rend le code brut
        };

        // Écriture du code brut dans l'entrée standard de rustfmt
        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(raw_code.as_bytes());
        }

        // Récupération de la sortie formatée
        match child.wait_with_output() {
            Ok(output) if output.status.success() => {
                String::from_utf8_lossy(&output.stdout).to_string()
            }
            _ => raw_code.to_string(), // Échec du formatage (syntaxe invalide ?), fallback
        }
    }
}

// --- STRUCTURES INTERNES POUR LE PARSING (MBSE 2.0) ---

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

    // Le pivot MBSE 2.0
    implementation: Option<ComponentImpl>,

    // Pour l'injection de logique
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
        // 1. Parsing typé du composant
        let component: ArcadiaComponent =
            serde_json::from_value(element.clone()).unwrap_or(ArcadiaComponent {
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

        let mut context = Context::new();
        context.insert("name", &component.name);
        context.insert("id", &component.id);
        context.insert(
            "description",
            &component.description.clone().unwrap_or_default(),
        );

        // 2. Extraction des fonctions pour l'IA
        let functions: Vec<String> = component
            .allocated_functions
            .iter()
            .map(|f| {
                // Supporte les références "ref:..." et les objets {"name": "..."}
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
        context.insert("functions", &functions);

        let mut files = Vec::new();

        // 3. Logique de Génération Conditionnelle
        if let Some(impl_specs) = &component.implementation {
            if impl_specs.technology == "Rust_Crate" {
                return self.generate_crate(
                    &component,
                    impl_specs,
                    &functions,
                    &context,
                    template_engine,
                );
            }
        }

        // 4. Fallback : Génération Legacy (Fichier unique)
        let content = template_engine.render("rust/actor", &context)?;

        // FORMATAGE AUTO
        let formatted_content = self.format_code(&content);

        files.push(GeneratedFile {
            path: PathBuf::from(format!("{}.rs", component.name.to_pascal_case())),
            content: formatted_content,
        });

        Ok(files)
    }
}

impl RustGenerator {
    /// Génère une structure de projet complète (Cargo.toml + lib.rs)
    fn generate_crate(
        &self,
        comp: &ArcadiaComponent,
        impl_specs: &ComponentImpl,
        functions: &[String],
        context: &Context,
        template_engine: &TemplateEngine,
    ) -> Result<Vec<GeneratedFile>> {
        let crate_name = impl_specs
            .artifact_name
            .clone()
            .unwrap_or_else(|| comp.name.to_snake_case());
        let mut files = Vec::new();

        // A. Génération de Cargo.toml (Pas besoin de rustfmt ici)
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

        // B. Génération de lib.rs avec injection Neuro-Symbolique
        let lib_content = template_engine
            .render("rust/lib", context)
            .unwrap_or_else(|_| {
                // Génération dynamique du squelette si pas de template
                let mut code = format!(
                    "//! {}\n//! Component ID: {}\n\n",
                    comp.description.as_deref().unwrap_or("No description"),
                    comp.id
                );

                for func in functions {
                    let fn_name = func.to_snake_case();
                    code.push_str(&format!("/// Implements system function: {}\n", func));
                    code.push_str(&format!("pub fn {}() {{\n", fn_name));

                    // Injection Point
                    code.push_str(&format!("    // AI_INJECTION_POINT: {}\n", fn_name));
                    code.push_str("    todo!(\"Waiting for neuro-symbolic implementation\");\n");
                    code.push_str("    // END_AI_INJECTION_POINT\n");

                    code.push_str("}\n\n");
                }
                code
            });

        // FORMATAGE AUTO
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
    use serde_json::json;

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

        // Vérifie que les éléments sont là
        assert!(content.contains("pub fn detect_objects()"));
        assert!(content.contains("// AI_INJECTION_POINT: detect_objects"));

        // NOTE: On ne peut pas garantir que rustfmt est installé sur l'environnement de test CI,
        // donc on ne fait pas d'assertion stricte sur l'indentation ici.
        // Mais le code doit compiler et ne pas être vide.
        assert!(!content.is_empty());
    }
}
