use crate::ai::llm::client::LlmClient;
use crate::code_generator::CodeGeneratorService;
use crate::json_db::storage::StorageEngine;
use std::path::PathBuf;
use std::sync::Arc;

/// Chemins structurels du projet GenAptitude
#[derive(Clone)]
pub struct AgentPaths {
    /// Le dossier contenant la DB du projet courant (PATH_RAISE_DOMAIN)
    pub domain_root: PathBuf,
    /// Le dossier contenant les schémas et templates (PATH_RAISE_DATASET)
    pub dataset_root: PathBuf,
}

/// Le contexte injecté dans chaque agent lors du `process`
#[derive(Clone)]
pub struct AgentContext {
    /// Moteur de persistance (accès aux collections OA, SA, LA, PA)
    pub db: Arc<StorageEngine>, // Arc pour le partage thread-safe

    /// Client IA pour la génération de texte/code
    pub llm: LlmClient,

    /// Service de génération de fichiers physiques
    pub codegen: Arc<CodeGeneratorService>,

    /// Configuration des chemins
    pub paths: AgentPaths,
}

impl AgentContext {
    pub fn new(
        db: Arc<StorageEngine>,
        llm: LlmClient,
        domain_root: PathBuf,
        dataset_root: PathBuf,
    ) -> Self {
        Self {
            db,
            llm,
            codegen: Arc::new(CodeGeneratorService::new(domain_root.clone())),
            paths: AgentPaths {
                domain_root,
                dataset_root,
            },
        }
    }
}
