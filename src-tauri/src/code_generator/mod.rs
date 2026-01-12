pub mod analyzers;
pub mod generators;
pub mod templates;

use self::analyzers::dependency_analyzer::DependencyAnalyzer;
use self::analyzers::injection_analyzer::InjectionAnalyzer;
use self::analyzers::Analyzer;
use self::generators::{
    cpp_gen::CppGenerator, rust_gen::RustGenerator, typescript_gen::TypeScriptGenerator,
    verilog_gen::VerilogGenerator, vhdl_gen::VhdlGenerator, LanguageGenerator,
};
use self::templates::template_engine::TemplateEngine;
use anyhow::{anyhow, Result};
use serde_json::Value;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TargetLanguage {
    Rust,
    TypeScript,
    Python,
    Cpp,
    Verilog,
    Vhdl,
}

pub struct CodeGeneratorService {
    root_path: PathBuf,
    template_engine: TemplateEngine,
    dep_analyzer: DependencyAnalyzer,
}

impl CodeGeneratorService {
    pub fn new(root_path: PathBuf) -> Self {
        Self {
            root_path,
            template_engine: TemplateEngine::new(),
            dep_analyzer: DependencyAnalyzer::new(),
        }
    }

    pub fn generate_for_element(
        &self,
        element: &Value,
        lang: TargetLanguage,
    ) -> Result<Vec<PathBuf>> {
        // 1. Analyse des dépendances (Modèle -> Imports)
        // Le résultat est stocké dans _analysis pour usage futur (injection dans le contexte)
        let _analysis = self.dep_analyzer.analyze(element)?;

        // 2. Sélection du générateur
        let generator: Box<dyn LanguageGenerator> = match lang {
            TargetLanguage::Rust => Box::new(RustGenerator::new()),
            TargetLanguage::Verilog => Box::new(VerilogGenerator::new()),
            TargetLanguage::Vhdl => Box::new(VhdlGenerator::new()),
            TargetLanguage::Cpp => Box::new(CppGenerator::new()),
            TargetLanguage::TypeScript => Box::new(TypeScriptGenerator::new()),
            TargetLanguage::Python => return Err(anyhow!("Générateur Python non implémenté")),
        };

        // 3. Génération "brute" (en mémoire)
        let mut files = generator.generate(element, &self.template_engine)?;
        let mut generated_paths = Vec::new();

        if !self.root_path.exists() {
            fs::create_dir_all(&self.root_path)?;
        }

        for file in &mut files {
            let full_path = self.root_path.join(&file.path);

            // 4. Préservation du code (Fichier existant -> Injections)
            if full_path.exists() {
                // On récupère le code utilisateur
                if let Ok(injections) = InjectionAnalyzer::extract_injections(&full_path) {
                    for (key, user_code) in injections {
                        // On réinjecte le code dans le contenu généré
                        // On cherche le marqueur par défaut dans le nouveau contenu
                        let marker = format!("AI_INJECTION_POINT: {}", key);

                        // Si le nouveau fichier contient le point d'injection
                        if file.content.contains(&marker) {
                            println!(
                                "Réinjection trouvée pour {} : {} octets",
                                key,
                                user_code.len()
                            );

                            // Remplacement : on remet le code utilisateur après le marqueur
                            file.content = file
                                .content
                                .replace(&marker, &format!("{}\n{}", marker, user_code));
                        }
                    }
                }
            }

            // 5. Écriture finale
            if let Some(parent) = full_path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(&full_path, &file.content)?;
            generated_paths.push(full_path);
        }

        Ok(generated_paths)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn test_integration_analyzers() {
        let dir = tempdir().unwrap();
        let root = dir.path().to_path_buf();
        let service = CodeGeneratorService::new(root.clone());

        // 1. Création d'un fichier existant avec du code utilisateur
        let existing_file = root.join("MyComponent.rs");
        {
            let mut f = fs::File::create(&existing_file).unwrap();
            // Correction ici : utilisation d'une string raw (r#""#) convertie en bytes
            // cela supporte les caractères UTF-8 comme 'é' dans "Généré"
            f.write_all(
                r#"
// Généré par Raise
struct MyComponent {}
// AI_INJECTION_POINT: Logic
fn custom() { println!("Preserved!"); }
// END_AI_INJECTION_POINT
            "#
                .as_bytes(),
            )
            .unwrap();
        }

        // 2. Régénération du même composant
        let element = json!({
            "name": "MyComponent",
            "id": "A1",
            "@type": "LogicalComponent"
        });

        let paths = service
            .generate_for_element(&element, TargetLanguage::Rust)
            .unwrap();
        assert_eq!(paths.len(), 1);

        // Vérification basique
        let new_content = fs::read_to_string(&paths[0]).unwrap();
        assert!(new_content.contains("pub struct MyComponent"));
    }
}
