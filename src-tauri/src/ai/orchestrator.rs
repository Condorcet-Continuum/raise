// FICHIER : src-tauri/src/ai/orchestrator.rs

use crate::ai::context::{
    conversation_manager::ConversationSession, memory_store::MemoryStore, rag::RagRetriever,
    retriever::SimpleRetriever,
};
use crate::ai::llm::client::{LlmBackend, LlmClient};
use crate::ai::nlp::parser::CommandType;
use crate::ai::world_model::{NeuroSymbolicEngine, WorldAction, WorldTrainer};
use crate::json_db::collections::manager::CollectionsManager;
use crate::json_db::storage::StorageEngine;
use crate::model_engine::types::{ArcadiaElement, ProjectModel};
use crate::utils::prelude::*;
use candle_nn::VarMap;

// --- IMPORTS AGENTS ---
use crate::ai::agents::intent_classifier::IntentClassifier;
// 🎯 NOUVEAU : On importe uniquement le DynamicAgent et les traits de base !
use crate::ai::agents::{
    dynamic_agent::DynamicAgent, Agent, AgentContext, AgentResult, CreatedArtifact,
};

pub struct AiOrchestrator {
    pub rag: RagRetriever,
    pub symbolic: SimpleRetriever,
    pub llm: LlmClient,
    pub session: ConversationSession,
    pub memory_store: MemoryStore,
    pub world_engine: SharedRef<NeuroSymbolicEngine>,
    #[allow(dead_code)]
    world_engine_path: PathBuf,

    // Référence au StorageEngine pour les Agents
    storage: Option<SharedRef<StorageEngine>>,
}

impl AiOrchestrator {
    /// Constructeur
    pub async fn new(
        model: ProjectModel,
        manager: &CollectionsManager<'_>,
        storage: SharedRef<StorageEngine>,
    ) -> RaiseResult<Self> {
        let app_config = AppConfig::get();

        let Some(domain_path) = app_config.get_path("PATH_RAISE_DOMAIN") else {
            raise_error!(
                "ERR_CONFIG_DOMAIN_PATH_MISSING",
                error = "PATH_RAISE_DOMAIN est manquant dans la configuration AppConfig",
                context = json_value!({
                    "required_key": "PATH_RAISE_DOMAIN",
                    "action": "initialize_domain_context"
                })
            );
        };
        let chats_path = domain_path.join("chats");
        let brain_path = domain_path.join("world_model.safetensors");

        let rag = RagRetriever::new(manager).await?;
        let symbolic = SimpleRetriever::new(model);
        let llm = LlmClient::new(manager).await?;

        let wm_config = app_config.world_model.clone();

        let world_engine = if brain_path.exists() {
            tracing::info!("🧠 [Orchestrator] Chargement du World Model existant...");
            NeuroSymbolicEngine::load_from_file(&brain_path, wm_config.clone())
                .await
                .unwrap_or_else(|e| {
                    tracing::error!("⚠️ Erreur chargement cerveau, réinitialisation: {}", e);
                    let vm = VarMap::new();
                    NeuroSymbolicEngine::new(wm_config.clone(), vm)
                        .expect("Echec fatal création WorldModel")
                })
        } else {
            tracing::info!("✨ [Orchestrator] Création d'un nouveau World Model vierge.");
            let vm = VarMap::new();
            NeuroSymbolicEngine::new(wm_config, vm)?
        };

        let memory_store = match MemoryStore::new(&chats_path).await {
            Ok(ms) => ms,
            Err(e) => raise_error!(
                "ERR_CHAT_MEMORY_STORE_INIT",
                error = e,
                context = json_value!({
                    "chats_path": chats_path.to_string_lossy(),
                    "component": "CHAT_SYSTEM"
                })
            ),
        };

        let session_id = "main_session";
        let session = memory_store.load_or_create(session_id).await?;

        Ok(Self {
            rag,
            symbolic,
            llm,
            session,
            memory_store,
            world_engine: SharedRef::new(world_engine),
            world_engine_path: brain_path,
            storage: Some(storage),
        })
    }

    /// Point d'entrée principal : Exécute une requête utilisateur via le système multi-agents
    pub async fn execute_workflow(&mut self, user_query: &str) -> RaiseResult<AgentResult> {
        let _rag_context = self.rag.retrieve(user_query, 3).await.unwrap_or_default();
        let _arcadia_context = self.symbolic.retrieve_context(user_query);

        let classifier = IntentClassifier::new(self.llm.clone());
        let mut current_intent = classifier.classify(user_query).await;
        // 🎯 L'intent renvoie maintenant une URN (ex: "ref:agents:handle:agent_software")
        let mut current_agent_urn = current_intent.recommended_agent_id().to_string();

        let session_scope = current_intent.default_session_scope();
        let global_session_id =
            AgentContext::generate_default_session_id("orchestrator", session_scope);

        let app_config = AppConfig::get();
        let Some(domain_path) = app_config.get_path("PATH_RAISE_DOMAIN") else {
            raise_error!(
                "ERR_CONFIG_DOMAIN_PATH_MISSING",
                error = "PATH_RAISE_DOMAIN est manquant dans la configuration AppConfig",
                context = json_value!({
                    "required_key": "PATH_RAISE_DOMAIN",
                    "action": "initialize_app_domain"
                })
            );
        };
        let dataset_path = app_config
            .get_path("PATH_RAISE_DATASET")
            .unwrap_or_else(|| domain_path.join("dataset"));

        let Some(storage_arc) = self.storage.clone() else {
            raise_error!(
                "ERR_AGENT_STORAGE_MISSING",
                error = "StorageEngine requis pour l'exécution des agents",
                context = json_value!({
                    "component": "AGENT_RUNNER",
                    "action": "execute_agent"
                })
            );
        };

        let mut hop_count = 0;
        const MAX_HOPS: i32 = 5;
        let mut accumulated_artifacts: Vec<CreatedArtifact> = Vec::new();
        let mut accumulated_messages: Vec<String> = Vec::new();

        loop {
            if hop_count >= MAX_HOPS {
                accumulated_messages
                    .push("⚠️ Limite de redirections entre agents atteinte.".to_string());
                break;
            }

            let ctx = AgentContext::new(
                &current_agent_urn,
                &global_session_id,
                storage_arc.clone(),
                self.llm.clone(),
                self.world_engine.clone(),
                domain_path.clone(),
                dataset_path.clone(),
            )
            .await;

            // 🎯 L'INSTANCIATION MAGIQUE DATA-DRIVEN EST ICI !
            tracing::info!(
                "🤖 Instanciation et Activation de l'Agent Dynamique: {}",
                current_agent_urn
            );
            let agent = DynamicAgent::new(&current_agent_urn);

            let result_opt = agent.process(&ctx, &current_intent).await?;

            if let Some(res) = result_opt {
                accumulated_artifacts.extend(res.artifacts);
                accumulated_messages.push(res.message);

                if let Some(acl_msg) = res.outgoing_message {
                    tracing::info!("📡 Délégation vers : {}", acl_msg.receiver);
                    current_agent_urn = acl_msg.receiver.clone();
                    current_intent = classifier.classify(&acl_msg.content).await;
                    hop_count += 1;
                    continue;
                } else {
                    break;
                }
            } else {
                break;
            }
        }

        Ok(AgentResult {
            message: accumulated_messages.join("\n\n---\n\n"),
            artifacts: accumulated_artifacts,
            outgoing_message: None,
        })
    }

    pub async fn ask(&mut self, query: &str) -> RaiseResult<String> {
        self.session.add_user_message(query);

        let rag_ctx = self.rag.retrieve(query, 3).await.unwrap_or_default();
        let arcadia_ctx = self.symbolic.retrieve_context(query);

        let mut prompt = format!("Demande Utilisateur : {}\n\n", query);

        if !rag_ctx.is_empty() {
            prompt.push_str(&rag_ctx);
            prompt.push('\n');
        }
        if !arcadia_ctx.contains("Aucun élément spécifique") {
            prompt.push_str(&arcadia_ctx);
            prompt.push('\n');
        }

        let response = self
            .llm
            .ask(
                LlmBackend::LocalLlama,
                "Tu es un expert système Arcadia. Utilise le contexte documentaire (RAG) et structurel (Modèle) fourni pour répondre avec précision.",
                &prompt,
            )
            .await?;

        self.session.add_ai_message(&response);
        let _ = self.memory_store.save_session(&self.session).await;

        Ok(response)
    }

    pub async fn reinforce_learning(
        &self,
        state_before: &ArcadiaElement,
        intent: CommandType,
        state_after: &ArcadiaElement,
    ) -> RaiseResult<f64> {
        let mut trainer = WorldTrainer::new(&self.world_engine, 0.01)?;
        let loss = trainer.train_step(state_before, WorldAction { intent }, state_after)?;
        let _ = self
            .world_engine
            .save_to_file(&self.world_engine_path)
            .await;
        Ok(loss)
    }

    pub async fn learn_document(&mut self, content: &str, source: &str) -> RaiseResult<usize> {
        self.rag.index_document(content, source).await
    }

    pub async fn clear_history(&mut self) -> RaiseResult<()> {
        self.session = ConversationSession::new(self.session.id.clone());
        let _ = self.memory_store.save_session(&self.session).await;
        Ok(())
    }
}

// =========================================================================
// TESTS UNITAIRES ET D'INTÉGRATION
// =========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::protocols::acl::{AclMessage, Performative};
    use crate::json_db::collections::manager::CollectionsManager;
    use crate::model_engine::types::NameType;
    use crate::utils::testing::*;

    fn get_hf_lock() -> &'static AsyncMutex<()> {
        static LOCK: StaticCell<AsyncMutex<()>> = StaticCell::new();
        LOCK.get_or_init(|| AsyncMutex::new(()))
    }

    fn make_element(id: &str) -> ArcadiaElement {
        ArcadiaElement {
            id: id.to_string(),
            name: NameType::default(),
            kind: "https://raise.io/ontology/arcadia/la#LogicalFunction".to_string(),
            description: None,
            properties: UnorderedMap::new(),
        }
    }

    async fn setup_mock_orchestrator_env() -> AgentDbSandbox {
        let sandbox = AgentDbSandbox::new().await;
        let manager = CollectionsManager::new(
            &sandbox.db,
            &sandbox.config.system_domain,
            &sandbox.config.system_db,
        );

        inject_mock_component(&manager, "llm",  json_value!({ "rust_tokenizer_file": "tokenizer.json", "rust_model_file": "qwen2.5-1.5b-instruct-q4_k_m.gguf" })).await;
        inject_mock_component(&manager, "nlp",  json_value!({ "model_name": "minilm", "rust_config_file": "config.json", "rust_tokenizer_file": "tokenizer.json", "rust_safetensors_file": "model.safetensors" })).await;

        sandbox
    }

    #[async_test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_orchestrator_init() {
        let _guard = get_hf_lock().lock().await;
        let sandbox = setup_mock_orchestrator_env().await;
        let manager = CollectionsManager::new(
            &sandbox.db,
            &sandbox.config.system_domain,
            &sandbox.config.system_db,
        );
        let orch = AiOrchestrator::new(ProjectModel::default(), &manager, sandbox.db.clone()).await;

        assert!(orch.is_ok(), "L'initialisation a échoué : {:?}", orch.err());
        assert_eq!(orch.unwrap().session.id, "main_session");
    }

    #[async_test]
    async fn test_full_acl_path() {
        let msg = AclMessage::new(
            Performative::Request,
            "hardware",
            "quality_manager",
            "Verify",
        );
        assert_eq!(msg.receiver, "quality_manager");
    }

    #[async_test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_learning_cycle() {
        let _guard = get_hf_lock().lock().await;
        let sandbox = setup_mock_orchestrator_env().await;
        let manager = CollectionsManager::new(
            &sandbox.db,
            &sandbox.config.system_domain,
            &sandbox.config.system_db,
        );
        let orch = AiOrchestrator::new(ProjectModel::default(), &manager, sandbox.db.clone())
            .await
            .unwrap();

        let loss = orch
            .reinforce_learning(&make_element("1"), CommandType::Create, &make_element("2"))
            .await;
        assert!(loss.is_ok(), "L'apprentissage a échoué : {:?}", loss.err());
    }

    #[async_test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_orchestrator_clear_history() {
        let _guard = get_hf_lock().lock().await;
        let sandbox = setup_mock_orchestrator_env().await;
        let manager = CollectionsManager::new(
            &sandbox.db,
            &sandbox.config.system_domain,
            &sandbox.config.system_db,
        );
        let mut orch = AiOrchestrator::new(ProjectModel::default(), &manager, sandbox.db.clone())
            .await
            .unwrap();

        orch.clear_history().await.unwrap();
        orch.session.add_user_message("Bonjour");
        orch.session.add_ai_message("Bonjour Humain");
        assert_eq!(orch.session.history.len(), 2);

        let clear_res = orch.clear_history().await;
        assert!(clear_res.is_ok());
        assert_eq!(orch.session.history.len(), 0);
    }

    #[async_test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_orchestrator_learn_document() {
        let _guard = get_hf_lock().lock().await;
        let sandbox = setup_mock_orchestrator_env().await;
        let manager = CollectionsManager::new(
            &sandbox.db,
            &sandbox.config.system_domain,
            &sandbox.config.system_db,
        );
        let mut orch = AiOrchestrator::new(ProjectModel::default(), &manager, sandbox.db.clone())
            .await
            .unwrap();

        let content = "Raise est une plateforme incroyable combinant RAG et modèles formels.";
        let res = orch.learn_document(content, "documentation.txt").await;
        assert!(res.is_ok());
        assert!(res.unwrap() > 0);
    }
}
