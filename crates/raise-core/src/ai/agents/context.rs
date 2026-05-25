// FICHIER : src-tauri/src/ai/agents/context.rs

use crate::utils::prelude::*;

use crate::ai::llm::client::LlmClient;
use crate::ai::world_model::NeuroSymbolicEngine;
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
    pub db: SharedRef<StorageEngine>,
    pub llm: LlmClient,
    pub codegen: SharedRef<CodeGeneratorService>,
    pub world_engine: SharedRef<NeuroSymbolicEngine>,
    pub paths: AgentPaths,
}

impl AgentContext {
    pub async fn new(
        agent_id: &str,
        session_id: &str,
        db: SharedRef<StorageEngine>,
        llm: LlmClient,
        world_engine: SharedRef<NeuroSymbolicEngine>,
        domain_root: PathBuf,
        dataset_root: PathBuf,
    ) -> RaiseResult<Self> {
        Ok(Self {
            agent_id: agent_id.to_string(),
            session_id: session_id.to_string(),
            db,
            llm,
            codegen: SharedRef::new(CodeGeneratorService::new(domain_root.clone())),
            world_engine,
            paths: AgentPaths {
                domain_root,
                dataset_root,
            },
        })
    }

    pub fn generate_default_session_id(agent_name: &str, workflow_id: &str) -> RaiseResult<String> {
        if agent_name.is_empty() || workflow_id.is_empty() {
            raise_error!(
                "ERR_AGENT_SESSION_ID_VOID",
                error = "L'ID de session ne peut pas être généré avec des identifiants vides."
            );
        }
        Ok(format!("session_{}_{}", agent_name, workflow_id))
    }
}

// ============================================================================
// TESTS UNITAIRES
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json_db::collections::manager::CollectionsManager;
    use crate::utils::testing::AgentDbSandbox;

    #[async_test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_context_initialization_with_session() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await?;

        // 🎯 FIX MOUNT POINTS : Utilisation du point de montage système pour injecter le composant
        let manager = CollectionsManager::new(
            &sandbox.db,
            &sandbox.config.mount_points.system.domain,
            &sandbox.config.mount_points.system.db,
        );

        // 🎯 Rigueur : Match sur la création du client LLM
        let llm = match LlmClient::new(
            &manager,
            sandbox.db.clone(),
            Some(sandbox.shared_engine.clone()),
        )
        .await
        {
            Ok(client) => client,
            Err(e) => panic!("Échec de l'initialisation du LlmClient : {:?}", e),
        };

        let domain_path = PathBuf::from("/data/domain");
        let dataset_path = PathBuf::from("/data/dataset");

        // 🎯 Rigueur : Match sur la création du World Model
        let world_engine = match NeuroSymbolicEngine::bootstrap(&manager).await {
            Ok(engine) => SharedRef::new(engine),
            Err(e) => panic!("Échec de l'initialisation du NeuroSymbolicEngine : {:?}", e),
        };

        let ctx = AgentContext::new(
            "agent_001",
            "session_abc",
            sandbox.db.clone(),
            llm,
            world_engine,
            domain_path.clone(),
            dataset_path.clone(),
        )
        .await?;

        assert_eq!(ctx.agent_id, "agent_001");
        assert_eq!(ctx.session_id, "session_abc");

        Ok(())
    }

    #[async_test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_empty_identifiers_validation() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await?;

        // 🎯 FIX MOUNT POINTS
        let manager = CollectionsManager::new(
            &sandbox.db,
            &sandbox.config.mount_points.system.domain,
            &sandbox.config.mount_points.system.db,
        );

        let llm = match LlmClient::new(
            &manager,
            sandbox.db.clone(),
            Some(sandbox.shared_engine.clone()),
        )
        .await
        {
            Ok(client) => client,
            Err(e) => panic!("Échec LLM : {:?}", e),
        };

        let world_engine = match NeuroSymbolicEngine::bootstrap(&manager).await {
            Ok(engine) => SharedRef::new(engine),
            Err(e) => panic!("Échec Neuro : {:?}", e),
        };

        let ctx = AgentContext::new(
            "",
            "",
            sandbox.db.clone(),
            llm,
            world_engine,
            PathBuf::new(),
            PathBuf::new(),
        )
        .await?;

        assert!(ctx.agent_id.is_empty());
        assert!(ctx.session_id.is_empty());

        Ok(())
    }

    #[test]
    fn test_session_id_format() -> RaiseResult<()> {
        let session = AgentContext::generate_default_session_id("data_agent", "WF-99")?;
        assert!(session.contains("data_agent"));
        assert!(session.contains("WF-99"));
        assert!(session.starts_with("session_"));

        Ok(())
    }
}
