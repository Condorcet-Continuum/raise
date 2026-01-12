use crate::ai::llm::client::LlmClient;
use crate::code_generator::CodeGeneratorService;
use crate::json_db::storage::StorageEngine;
use std::path::PathBuf;
use std::sync::Arc;

/// Chemins structurels du projet RAISE
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

// --- TESTS UNITAIRES ---
#[cfg(test)]
mod tests {
    use super::*;
    use crate::json_db::storage::JsonDbConfig;

    // Helper pour mocker les dépendances lourdes (StorageEngine)
    fn mock_storage() -> Arc<StorageEngine> {
        let config = JsonDbConfig::new(PathBuf::from("/tmp/test_db"));
        Arc::new(StorageEngine::new(config))
    }

    #[test]
    fn test_context_initialization() {
        let db = mock_storage();
        let llm = LlmClient::new("http://dummy", "key", None);
        let domain_path = PathBuf::from("/data/domain");
        let dataset_path = PathBuf::from("/data/dataset");

        let ctx = AgentContext::new(db, llm, domain_path.clone(), dataset_path.clone());

        assert_eq!(ctx.paths.domain_root, domain_path);
        assert_eq!(ctx.paths.dataset_root, dataset_path);
        // On vérifie que le codegen a bien reçu le chemin racine
        // (Note: on ne peut pas inspecter codegen.root_path car privé, mais l'instanciation sans panic est un bon signe)
    }
}
