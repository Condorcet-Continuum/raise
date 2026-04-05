// src-tauri/src/ai/agents/context.rs

use crate::utils::prelude::*;

use crate::ai::llm::client::LlmClient;
use crate::ai::world_model::NeuroSymbolicEngine;
use crate::code_generator::CodeGeneratorService;
use crate::json_db::storage::StorageEngine; // 🎯 IMPORT DU CERVEAU

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
    pub db: SharedRef<StorageEngine>,
    pub llm: LlmClient,
    pub codegen: SharedRef<CodeGeneratorService>,
    pub world_engine: SharedRef<NeuroSymbolicEngine>, // 🎯 INJECTION DU CERVEAU
    pub paths: AgentPaths,
}

impl AgentContext {
    pub async fn new(
        agent_id: &str,
        session_id: &str,
        db: SharedRef<StorageEngine>,
        llm: LlmClient,
        world_engine: SharedRef<NeuroSymbolicEngine>, // 🎯 NOUVEAU PARAMÈTRE
        domain_root: PathBuf,
        dataset_root: PathBuf,
    ) -> Self {
        Self {
            agent_id: agent_id.to_string(),
            session_id: session_id.to_string(),
            db,
            llm,
            codegen: SharedRef::new(CodeGeneratorService::new(domain_root.clone())),
            world_engine, // 🎯 ASSIGNATION
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
    use crate::utils::testing::{inject_mock_component, AgentDbSandbox};
    use candle_nn::VarMap; // 🎯 Import pour le mock du World Model

    #[async_test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_context_initialization_with_session() {
        let sandbox = AgentDbSandbox::new().await;
        let manager = CollectionsManager::new(
            &sandbox.db,
            &sandbox.config.system_domain,
            &sandbox.config.system_db,
        );

        inject_mock_component(&manager, "llm", json_value!({})).await;

        let llm = LlmClient::new(&manager).await.unwrap();
        let domain_path = PathBuf::from("/data/domain");
        let dataset_path = PathBuf::from("/data/dataset");

        // 🎯 Mock du World Model pour le test
        let wm_config = crate::utils::data::config::WorldModelConfig::default();
        let world_engine =
            SharedRef::new(NeuroSymbolicEngine::new(wm_config, VarMap::new()).unwrap());

        let ctx = AgentContext::new(
            "agent_001",
            "session_abc",
            sandbox.db.clone(),
            llm,
            world_engine, // 🎯 Injection
            domain_path.clone(),
            dataset_path.clone(),
        )
        .await;

        assert_eq!(ctx.agent_id, "agent_001");
        assert_eq!(ctx.session_id, "session_abc");
    }

    #[async_test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_empty_identifiers_validation() {
        let sandbox = AgentDbSandbox::new().await;
        let manager = CollectionsManager::new(
            &sandbox.db,
            &sandbox.config.system_domain,
            &sandbox.config.system_db,
        );

        inject_mock_component(&manager, "llm", json_value!({})).await;

        let llm = LlmClient::new(&manager).await.unwrap();

        // 🎯 Mock du World Model pour le test
        let wm_config = crate::utils::data::config::WorldModelConfig::default();
        let world_engine =
            SharedRef::new(NeuroSymbolicEngine::new(wm_config, VarMap::new()).unwrap());

        let ctx = AgentContext::new(
            "",
            "",
            sandbox.db.clone(),
            llm,
            world_engine, // 🎯 Injection
            PathBuf::new(),
            PathBuf::new(),
        )
        .await;

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
