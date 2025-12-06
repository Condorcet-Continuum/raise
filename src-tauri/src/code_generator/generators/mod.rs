use anyhow::Result;
use serde_json::Value;
use std::path::PathBuf;

// On expose le générateur Rust
pub mod rust_gen;

pub struct GeneratedFile {
    pub path: PathBuf,
    pub content: String,
}

pub trait LanguageGenerator {
    /// Génère une liste de fichiers à partir d'un élément du modèle (JSON)
    fn generate(&self, element: &Value) -> Result<Vec<GeneratedFile>>;
}

#[cfg(test)]
mod tests;
