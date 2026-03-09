// FICHIER : src-tauri/src/ai/orchestrator.rs

use crate::ai::context::{
    conversation_manager::ConversationSession, memory_store::MemoryStore, rag::RagRetriever,
    retriever::SimpleRetriever,
};
use crate::ai::llm::client::{LlmBackend, LlmClient};
use crate::ai::nlp::parser::CommandType;
use crate::ai::world_model::{NeuroSymbolicEngine, WorldAction, WorldTrainer};
use crate::model_engine::types::{ArcadiaElement, ProjectModel};
use candle_nn::VarMap;

use crate::json_db::storage::StorageEngine;
use crate::utils::prelude::*;

// --- IMPORTS AGENTS ---
use crate::ai::agents::intent_classifier::IntentClassifier;
use crate::ai::agents::{
    business_agent::BusinessAgent, data_agent::DataAgent, epbs_agent::EpbsAgent,
    hardware_agent::HardwareAgent, software_agent::SoftwareAgent, system_agent::SystemAgent,
    transverse_agent::TransverseAgent, Agent, AgentContext, AgentResult, CreatedArtifact,
};

pub struct AiOrchestrator {
    pub rag: RagRetriever,
    pub symbolic: SimpleRetriever,
    pub llm: LlmClient,
    pub session: ConversationSession,
    pub memory_store: MemoryStore,
    pub world_engine: NeuroSymbolicEngine,
    #[allow(dead_code)]
    world_engine_path: PathBuf,

    // Référence au StorageEngine pour les Agents (Optionnel pour le mode léger, requis pour les agents)
    storage: Option<SharedRef<StorageEngine>>,
}

impl AiOrchestrator {
    /// Constructeur
    pub async fn new(
        model: ProjectModel,
        storage_engine: Option<SharedRef<StorageEngine>>,
    ) -> RaiseResult<Self> {
        let app_config = AppConfig::get();

        // Sécurité : On récupère le chemin du domaine via la config globale
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

        // 🎯 1. INSTANCIATION CENTRALE DE LA BASE DE DONNÉES
        let actual_storage = storage_engine.unwrap_or_else(|| {
            let storage_cfg = crate::json_db::storage::JsonDbConfig::new(domain_path.clone());
            SharedRef::new(StorageEngine::new(storage_cfg))
        });

        let manager = crate::json_db::collections::manager::CollectionsManager::new(
            &actual_storage,
            &app_config.system_domain,
            &app_config.system_db,
        );

        // 🎯 2. INJECTION DU MANAGER AUX SOUS-MOTEURS
        let rag = RagRetriever::new(&manager).await?;
        let symbolic = SimpleRetriever::new(model);
        let llm = LlmClient::new(&manager).await?;

        // Configuration du World Model (Neuro-Symbolique)
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

        // Gestion de la mémoire de conversation
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
            world_engine,
            world_engine_path: brain_path,
            storage: Some(actual_storage),
        })
    }

    /// Factory simple pour instancier les agents spécialisés
    fn create_agent(&self, agent_id: &str) -> Option<Box<dyn Agent>> {
        match agent_id {
            "business_agent" | "business_analyst" => Some(Box::new(BusinessAgent::new())),
            "system_agent" | "system_architect" => Some(Box::new(SystemAgent::new())),
            "software_agent" | "software_engineer" => Some(Box::new(SoftwareAgent::new())),
            "hardware_agent" | "hardware_architect" => Some(Box::new(HardwareAgent::new())),
            "epbs_agent" | "configuration_manager" => Some(Box::new(EpbsAgent::new())),
            "data_agent" | "data_architect" => Some(Box::new(DataAgent::new())),
            "transverse_agent" | "quality_manager" => Some(Box::new(TransverseAgent::new())),
            _ => None,
        }
    }

    /// Point d'entrée principal : Exécute une requête utilisateur via le système multi-agents
    pub async fn execute_workflow(&mut self, user_query: &str) -> RaiseResult<AgentResult> {
        let _rag_context = self.rag.retrieve(user_query, 3).await.unwrap_or_default();
        let _arcadia_context = self.symbolic.retrieve_context(user_query);

        let classifier = IntentClassifier::new(self.llm.clone());
        let mut current_intent = classifier.classify(user_query).await;
        let mut current_agent_id = current_intent.recommended_agent_id().to_string();

        if current_agent_id == "orchestrator_agent" {
            let response = self.ask(user_query).await?;
            return Ok(AgentResult::text(response));
        }

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

            // 🎯 3. LE .await MANQUANT AJOUTÉ ICI !
            let ctx = AgentContext::new(
                &current_agent_id,
                &global_session_id,
                storage_arc.clone(),
                self.llm.clone(), // Le contexte reçoit directement le LLM instancié !
                domain_path.clone(),
                dataset_path.clone(),
            )
            .await;

            if let Some(agent) = self.create_agent(&current_agent_id) {
                tracing::info!("🤖 Activation Agent: {}", current_agent_id);

                let result_opt = agent.process(&ctx, &current_intent).await?;

                if let Some(res) = result_opt {
                    accumulated_artifacts.extend(res.artifacts);
                    accumulated_messages.push(res.message);

                    if let Some(acl_msg) = res.outgoing_message {
                        tracing::info!("📡 Délégation vers : {}", acl_msg.receiver);
                        current_agent_id = acl_msg.receiver.clone();
                        current_intent = classifier.classify(&acl_msg.content).await;
                        hop_count += 1;
                        continue;
                    } else {
                        break;
                    }
                } else {
                    break;
                }
            } else {
                accumulated_messages.push(format!(
                    "❌ Agent inconnu ou non implémenté : {}",
                    current_agent_id
                ));
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

    // 🎯 HELPER POUR INJECTER LA BDD DE TEST AVANT L'ORCHESTRATEUR
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
        let orch = AiOrchestrator::new(ProjectModel::default(), Some(sandbox.db.clone())).await;

        assert!(
            orch.is_ok(),
            "L'initialisation de l'orchestrateur a échoué : {:?}",
            orch.err()
        );
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
        let orch = AiOrchestrator::new(ProjectModel::default(), Some(sandbox.db.clone()))
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
    async fn test_orchestrator_agent_factory() {
        let _guard = get_hf_lock().lock().await;
        let sandbox = setup_mock_orchestrator_env().await;
        let orch = AiOrchestrator::new(ProjectModel::default(), Some(sandbox.db.clone()))
            .await
            .unwrap();

        assert!(orch.create_agent("business_agent").is_some());
        assert!(orch.create_agent("system_architect").is_some());
        assert!(orch.create_agent("unknown_hacker_agent").is_none());
    }

    #[async_test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_orchestrator_clear_history() {
        let _guard = get_hf_lock().lock().await;
        let sandbox = setup_mock_orchestrator_env().await;
        let mut orch = AiOrchestrator::new(ProjectModel::default(), Some(sandbox.db.clone()))
            .await
            .unwrap();

        // On purge l'historique résiduel potentiellement laissé par les tests précédents
        // qui ont utilisé la même base de données mockée.
        orch.clear_history().await.unwrap();

        // Maintenant on est sûr de partir de 0 !
        orch.session.add_user_message("Bonjour l'IA");
        orch.session.add_ai_message("Bonjour Humain");
        assert_eq!(orch.session.history.len(), 2);

        // Test du nettoyage
        let clear_res = orch.clear_history().await;
        assert!(clear_res.is_ok());
        assert_eq!(orch.session.history.len(), 0);
        assert_eq!(orch.session.id, "main_session");
    }

    #[async_test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_orchestrator_learn_document() {
        let _guard = get_hf_lock().lock().await;
        let sandbox = setup_mock_orchestrator_env().await;
        let mut orch = AiOrchestrator::new(ProjectModel::default(), Some(sandbox.db.clone()))
            .await
            .unwrap();

        let content = "Raise est une plateforme incroyable combinant RAG et modèles formels.";
        let res = orch.learn_document(content, "documentation.txt").await;

        assert!(
            res.is_ok(),
            "L'apprentissage de document a échoué : {:?}",
            res.err()
        );
        assert!(res.unwrap() > 0);
    }
}
