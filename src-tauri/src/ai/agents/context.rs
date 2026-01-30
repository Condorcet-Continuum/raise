// src-tauri/src/ai/agents/context.rs

use crate::ai::llm::client::LlmClient;
use crate::code_generator::CodeGeneratorService;
use crate::json_db::storage::StorageEngine;
use std::path::PathBuf;
use std::sync::Arc;

/// Chemins structurels du projet RAISE
#[derive(Clone)]
pub struct AgentPaths {
    pub domain_root: PathBuf,
    pub dataset_root: PathBuf,
}

/// Le contexte injecté dans chaque agent lors du `process`
#[derive(Clone)]
pub struct AgentContext {
    pub agent_id: String,
    pub session_id: String,
    pub db: Arc<StorageEngine>,
    pub llm: LlmClient,
    pub codegen: Arc<CodeGeneratorService>,
    pub paths: AgentPaths,
}

impl AgentContext {
    pub fn new(
        agent_id: &str,
        session_id: &str,
        db: Arc<StorageEngine>,
        llm: LlmClient,
        domain_root: PathBuf,
        dataset_root: PathBuf,
    ) -> Self {
        Self {
            agent_id: agent_id.to_string(),
            session_id: session_id.to_string(),
            db,
            llm,
            codegen: Arc::new(CodeGeneratorService::new(domain_root.clone())),
            paths: AgentPaths {
                domain_root,
                dataset_root,
            },
        }
    }

    pub fn generate_default_session_id(agent_name: &str, workflow_id: &str) -> String {
        format!("session_{}_{}", agent_name, workflow_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json_db::storage::JsonDbConfig;

    fn mock_storage() -> Arc<StorageEngine> {
        let config = JsonDbConfig::new(PathBuf::from("/tmp/test_db_agent_v2"));
        Arc::new(StorageEngine::new(config))
    }

    #[test]
    fn test_context_initialization_with_session() {
        let db = mock_storage();
        let llm = LlmClient::new("http://dummy", "key", None);
        let domain_path = PathBuf::from("/data/domain");
        let dataset_path = PathBuf::from("/data/dataset");

        let ctx = AgentContext::new(
            "agent_001",
            "session_abc",
            db,
            llm,
            domain_path.clone(),
            dataset_path.clone(),
        );

        assert_eq!(ctx.agent_id, "agent_001");
        assert_eq!(ctx.session_id, "session_abc");
    }

    #[test]
    fn test_empty_identifiers_validation() {
        // Test de sécurité pour s'assurer que le système accepte (techniquement) les IDs vides
        // même si la logique métier devrait les éviter.
        let db = mock_storage();
        let llm = LlmClient::new("http://dummy", "key", None);
        let ctx = AgentContext::new("", "", db, llm, PathBuf::new(), PathBuf::new());
        assert!(ctx.agent_id.is_empty());
        assert!(ctx.session_id.is_empty());
    }

    #[test]
    fn test_session_id_format() {
        let session = AgentContext::generate_default_session_id("data_agent", "WF-99");
        assert!(session.contains("data_agent"));
        assert!(session.contains("WF-99"));
        assert!(session.starts_with("session_"));
    }
}
