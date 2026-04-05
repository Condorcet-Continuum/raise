// FICHIER : src-tauri/src/code_generator/toolchains/mod.rs

pub mod rust;

use crate::utils::prelude::*;
use async_trait::async_trait; // Utilisé massivement dans les architectures Tokio

/// ⚙️ Contrat universel pour les validateurs de code physique
#[async_trait]
pub trait ToolchainStrategy: Send + Sync {
    /// Formate le code source physique
    async fn format(&self, path: &Path) -> RaiseResult<()>;
    /// Vérifie la syntaxe (AST / Compilation statique)
    async fn check(&self, module_name: &str, cwd: Option<&Path>) -> RaiseResult<()>;
    /// Exécute les tests unitaires / métier
    async fn test(&self, module_name: &str, cwd: Option<&Path>) -> RaiseResult<()>;
}

/// 🏭 Usine pour instancier la bonne toolchain
pub struct ToolchainFactory;

impl ToolchainFactory {
    /// Déduit la toolchain à utiliser selon l'extension du fichier
    pub fn for_extension(ext: &str) -> Option<Box<dyn ToolchainStrategy>> {
        match ext {
            "rs" => Some(Box::new(rust::RustToolchain)),
            // "py" => Some(Box::new(python::PythonToolchain)), // 🚀 Prêt pour le futur !
            // "ts" => Some(Box::new(typescript::TypeScriptToolchain)),
            _ => None,
        }
    }
}
