// src-tauri/src/ai/agents/context.rs

use crate::utils::{io::PathBuf, Arc};

use crate::ai::llm::client::LlmClient;
use crate::code_generator::CodeGeneratorService;
use crate::json_db::storage::StorageEngine;

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
    pub async fn new(
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
    use crate::json_db::collections::manager::CollectionsManager;
    use crate::json_db::storage::JsonDbConfig;
    use crate::utils::config::test_mocks::inject_mock_config;
    use crate::utils::config::AppConfig;

    async fn setup_test_env() -> (Arc<StorageEngine>, AppConfig) {
        inject_mock_config();
        let config = AppConfig::get();
        let storage_cfg = JsonDbConfig::new(PathBuf::from("/tmp/test_db_agent_v2"));
        (Arc::new(StorageEngine::new(storage_cfg)), config.clone())
    }

    #[tokio::test] // 🎯 Les tests deviennent asynchrones
    #[serial_test::serial] // Protection RTX 5060 en local
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_context_initialization_with_session() {
        let (db, config) = setup_test_env().await;
        let manager = CollectionsManager::new(&db, &config.system_domain, &config.system_db);
        manager.init_db().await.unwrap();

        // Injection du composant LLM pour le test
        crate::utils::config::test_mocks::inject_mock_component(
            &manager,
            "llm", 
            crate::utils::json::json!({ "rust_tokenizer_file": "tokenizer.json", "rust_model_file": "qwen2.5-1.5b-instruct-q4_k_m.gguf" })
        ).await;

        let llm = LlmClient::new(&manager).await.unwrap();
        let domain_path = PathBuf::from("/data/domain");
        let dataset_path = PathBuf::from("/data/dataset");

        let ctx = AgentContext::new(
            "agent_001",
            "session_abc",
            db.clone(),
            llm,
            domain_path.clone(),
            dataset_path.clone(),
        )
        .await; // 🎯 Le fameux .await manquant !

        assert_eq!(ctx.agent_id, "agent_001");
        assert_eq!(ctx.session_id, "session_abc");
    }

    #[tokio::test] // 🎯 Les tests deviennent asynchrones
    #[serial_test::serial] // Protection RTX 5060 en local
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_empty_identifiers_validation() {
        let (db, config) = setup_test_env().await;
        let manager = CollectionsManager::new(&db, &config.system_domain, &config.system_db);
        manager.init_db().await.unwrap();

        crate::utils::config::test_mocks::inject_mock_component(
            &manager,
            "llm", 
            crate::utils::json::json!({ "rust_tokenizer_file": "tokenizer.json", "rust_model_file": "qwen2.5-1.5b-instruct-q4_k_m.gguf" })
        ).await;

        let llm = LlmClient::new(&manager).await.unwrap();
        let ctx = AgentContext::new("", "", db.clone(), llm, PathBuf::new(), PathBuf::new()).await;

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
