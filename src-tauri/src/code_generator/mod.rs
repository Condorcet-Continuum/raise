// Déclaration des sous-modules
pub mod analyzers;
pub mod generators;
pub mod templates; // Sera enrichi plus tard // Sera enrichi plus tard

use self::generators::{rust_gen::RustGenerator, LanguageGenerator};
use anyhow::{anyhow, Result};
use serde_json::Value;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy)]
pub enum TargetLanguage {
    Rust,
    TypeScript,
    Python,
}

pub struct CodeGeneratorService {
    root_path: PathBuf,
}

impl CodeGeneratorService {
    pub fn new(root_path: PathBuf) -> Self {
        Self { root_path }
    }

    /// Génère le code pour un élément donné dans le langage cible
    pub fn generate_for_element(
        &self,
        element: &Value,
        lang: TargetLanguage,
    ) -> Result<Vec<PathBuf>> {
        // Factory : on choisit le bon générateur
        let generator: Box<dyn LanguageGenerator> = match lang {
            TargetLanguage::Rust => Box::new(RustGenerator::new()),
            _ => {
                return Err(anyhow!(
                    "Ce langage n'est pas encore supporté par le générateur de templates."
                ))
            }
        };

        let files = generator.generate(element)?;
        let mut generated_paths = Vec::new();

        // Création du dossier cible si inexistant
        if !self.root_path.exists() {
            fs::create_dir_all(&self.root_path)?;
        }

        // Écriture des fichiers
        for file in files {
            let full_path = self.root_path.join(file.path);

            // Création des sous-dossiers éventuels
            if let Some(parent) = full_path.parent() {
                fs::create_dir_all(parent)?;
            }

            fs::write(&full_path, &file.content)?;
            generated_paths.push(full_path);
        }

        Ok(generated_paths)
    }
}
