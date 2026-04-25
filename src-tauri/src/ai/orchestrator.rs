// FICHIER : src-tauri/src/ai/orchestrator.rs

use crate::ai::context::{
    conversation_manager::ConversationSession, memory_store::MemoryStore, rag::RagRetriever,
    retriever::SimpleRetriever,
};
use crate::ai::llm::client::{LlmBackend, LlmClient};
use crate::ai::nlp::parser::CommandType;
use crate::ai::world_model::engine::WorldModelConfig;
use crate::ai::world_model::{NeuroSymbolicEngine, WorldAction, WorldTrainer};
use crate::json_db::collections::manager::CollectionsManager;
use crate::json_db::storage::StorageEngine;
use crate::model_engine::types::{ArcadiaElement, ProjectModel};
use crate::utils::prelude::*;

// --- IMPORTS AGENTS ---
use crate::ai::agents::intent_classifier::IntentClassifier;
use crate::ai::agents::{dynamic_agent::DynamicAgent, Agent, AgentContext, AgentResult};

/// Chef d'orchestre du système IA RAISE.
/// Gère le cycle de vie hybride : RAG sémantique, Inférence LLM et World Model Neuro-Symbolique.
pub struct AiOrchestrator {
    pub rag: RagRetriever,
    pub symbolic: SimpleRetriever,
    pub llm: LlmClient,
    pub session: ConversationSession,
    pub memory_store: MemoryStore,
    pub world_engine: SharedRef<NeuroSymbolicEngine>,

    pub space: String,
    pub db_name: String,
    storage: Option<SharedRef<StorageEngine>>,
}

impl AiOrchestrator {
    /// Initialise l'orchestrateur en résolvant les composants via les Mount Points système.
    pub async fn new(
        model: ProjectModel,
        manager: &CollectionsManager<'_>,
        storage: SharedRef<StorageEngine>,
    ) -> RaiseResult<Self> {
        // 1. Initialisation des composants RAG et LLM (Gérés par leurs propres façades)
        let rag = RagRetriever::new(manager).await?;
        let symbolic = SimpleRetriever::new(model);
        let llm = LlmClient::new(manager).await?;
        let world_engine = match NeuroSymbolicEngine::bootstrap(manager).await {
            Ok(engine) => engine,
            Err(e) => {
                user_warn!(
                    "WRN_WORLD_MODEL_LOAD_FAILED",
                    json_value!({ "error": e.to_string(), "hint": "Modèle corrompu ou absent. Démarrage à froid." })
                );

                // Récupération de la configuration (Zéro Dette)
                let wm_settings = AppConfig::get_runtime_settings(
                    manager,
                    "ref:components:handle:ai_world_model",
                )
                .await?;
                let wm_config: WorldModelConfig = match json::deserialize_from_value(wm_settings) {
                    Ok(cfg) => cfg,
                    Err(err) => raise_error!("ERR_WM_CONFIG_DESERIALIZE", error = err.to_string()),
                };

                // Initialisation d'un modèle vierge en mémoire
                NeuroSymbolicEngine::new_empty(wm_config)?
            }
        };

        // 3. Initialisation du stockage de mémoire conversationnelle
        let memory_store = match MemoryStore::new(manager).await {
            Ok(ms) => ms,
            Err(e) => raise_error!(
                "ERR_CHAT_MEMORY_STORE_INIT",
                error = e.to_string(),
                context = json_value!({ "domain": manager.space })
            ),
        };

        let session_id = "main_session";
        let session = memory_store.load_or_create(manager, session_id).await?;

        Ok(Self {
            rag,
            symbolic,
            llm,
            session,
            memory_store,
            world_engine: SharedRef::new(world_engine),
            space: manager.space.to_string(),
            db_name: manager.db.to_string(),
            storage: Some(storage),
        })
    }

    /// Exécute un workflow multi-agents complet avec routage d'intention.
    pub async fn execute_workflow(&mut self, user_query: &str) -> RaiseResult<AgentResult> {
        let app_config = AppConfig::get();
        let storage_arc = match self.storage.clone() {
            Some(s) => s,
            None => raise_error!(
                "ERR_AGENT_STORAGE_MISSING",
                error = "StorageEngine non injecté"
            ),
        };

        // Utilisation des Mount Points pour reconstruire le manager technique
        let _manager = CollectionsManager::new(
            storage_arc.as_ref(),
            &app_config.mount_points.system.domain,
            &app_config.mount_points.system.db,
        );

        // Classification de l'intention via LLM
        let classifier = IntentClassifier::new(self.llm.clone());
        let mut current_intent = classifier.classify(user_query).await;
        let mut current_agent_urn = current_intent.recommended_agent_id().to_string();

        let session_scope = current_intent.default_session_scope();
        let global_session_id =
            AgentContext::generate_default_session_id("orchestrator", session_scope)?;

        // Résolution déterministe des chemins via AppConfig
        let domain_path = match app_config.get_path("PATH_RAISE_DOMAIN") {
            Some(p) => p,
            None => raise_error!(
                "ERR_CONFIG_PATH_MISSING",
                error = "PATH_RAISE_DOMAIN non défini"
            ),
        };
        let dataset_path = app_config
            .get_path("PATH_RAISE_DATASET")
            .unwrap_or_else(|| domain_path.join("dataset"));

        let mut hop_count = 0;
        const MAX_HOPS: i32 = 5;
        let mut accumulated_artifacts = Vec::new();
        let mut accumulated_messages = Vec::new();

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
            .await?;

            let agent = DynamicAgent::new(&current_agent_urn);
            match agent.process(&ctx, &current_intent).await? {
                Some(res) => {
                    accumulated_artifacts.extend(res.artifacts);
                    accumulated_messages.push(res.message);

                    if let Some(acl_msg) = res.outgoing_message {
                        current_agent_urn = acl_msg.receiver.clone();
                        current_intent = classifier.classify(&acl_msg.content).await;
                        hop_count += 1;
                        continue;
                    } else {
                        break;
                    }
                }
                None => break,
            }
        }

        Ok(AgentResult {
            message: accumulated_messages.join("\n\n---\n\n"),
            artifacts: accumulated_artifacts,
            outgoing_message: None,
            xai_frame: None,
        })
    }

    /// Interface "Ask" simplifiée pour le mode conversationnel.
    pub async fn ask(&mut self, query: &str) -> RaiseResult<String> {
        self.session.add_user_message(query);
        let app_config = AppConfig::get();

        let storage_arc = match &self.storage {
            Some(s) => s,
            None => raise_error!("ERR_STORAGE_MISSING"),
        };

        let manager = CollectionsManager::new(
            storage_arc.as_ref(),
            &app_config.mount_points.system.domain,
            &app_config.mount_points.system.db,
        );

        // Recherche hybride RAG + Symbolique
        let rag_ctx: String = self
            .rag
            .retrieve(&manager, query, 3)
            .await
            .unwrap_or_default();

        let arcadia_ctx = self.symbolic.retrieve_context(query);

        let mut prompt = format!("Demande Utilisateur : {}\n\n", query);
        if !rag_ctx.is_empty() {
            prompt.push_str(&format!("Contexte RAG : {}\n", rag_ctx));
        }
        if !arcadia_ctx.contains("Aucun élément") {
            prompt.push_str(&format!("Modèle Arcadia : {}\n", arcadia_ctx));
        }

        let response = self
            .llm
            .ask(
                LlmBackend::LocalLlama,
                "Tu es un expert système Arcadia RAISE.",
                &prompt,
            )
            .await?;

        self.session.add_ai_message(&response);
        let _ = self
            .memory_store
            .save_session(&manager, &self.session)
            .await;

        Ok(response)
    }

    /// Apprentissage par renforcement du World Model Arcadia.
    pub async fn reinforce_learning(
        &self,
        state_before: &ArcadiaElement,
        intent: CommandType,
        state_after: &ArcadiaElement,
    ) -> RaiseResult<f64> {
        let mut trainer = match WorldTrainer::new(&self.world_engine, 0.01) {
            Ok(t) => t,
            Err(e) => raise_error!("ERR_WM_TRAINER_INIT", error = e.to_string()),
        };

        let loss = trainer.train_step(state_before, WorldAction { intent }, state_after)?;

        if let Some(storage_arc) = &self.storage {
            let manager = CollectionsManager::new(storage_arc.as_ref(), &self.space, &self.db_name);
            match self.world_engine.save(&manager).await {
                Ok(_) => (),
                Err(e) => user_error!("ERR_WM_SAVE_FAIL", json_value!({"error": e.to_string()})),
            }
        }

        Ok(loss)
    }

    pub async fn learn_document(&mut self, content: &str, source: &str) -> RaiseResult<usize> {
        let app_config = AppConfig::get();
        let storage_arc = match &self.storage {
            Some(s) => s,
            None => raise_error!("ERR_STORAGE_MISSING"),
        };
        let manager = CollectionsManager::new(
            storage_arc.as_ref(),
            &app_config.mount_points.system.domain,
            &app_config.mount_points.system.db,
        );
        self.rag.index_document(&manager, content, source).await
    }

    pub async fn clear_history(&mut self) -> RaiseResult<()> {
        self.session = ConversationSession::new(self.session.id.clone());
        let app_config = AppConfig::get();
        let storage_arc = match &self.storage {
            Some(s) => s,
            None => raise_error!("ERR_STORAGE_MISSING"),
        };
        let manager = CollectionsManager::new(
            storage_arc.as_ref(),
            &app_config.mount_points.system.domain,
            &app_config.mount_points.system.db,
        );
        let _ = self
            .memory_store
            .save_session(&manager, &self.session)
            .await;
        Ok(())
    }
}

// =========================================================================
// TESTS UNITAIRES (Validation Mount Points & Résilience)
// =========================================================================

#[cfg(test)]
mod tests {
    use super::*;
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
            properties: UnorderedMap::new(),
        }
    }

    #[async_test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_orchestrator_lifecycle() -> RaiseResult<()> {
        let _guard = get_hf_lock().lock().await;

        let sandbox = AgentDbSandbox::new().await?;
        let config = AppConfig::get();
        let manager = CollectionsManager::new(
            &sandbox.db,
            &config.mount_points.system.domain,
            &config.mount_points.system.db,
        );

        // 1. TEST D'INITIALISATION RÉSILIENTE
        let mut orch =
            AiOrchestrator::new(ProjectModel::default(), &manager, sandbox.db.clone()).await?;
        assert_eq!(orch.session.id, "main_session");

        // 2. TEST DE L'APPRENTISSAGE RAG (Persistance DB)
        let content = "RAISE fusionne MBSE et Deep Learning.";
        let res = orch.learn_document(content, "doc.txt").await?;
        assert!(res > 0);

        // 3. TEST DU WORLD MODEL (Apprentissage Renforcé)
        let loss = orch
            .reinforce_learning(&make_element("1"), CommandType::Create, &make_element("2"))
            .await?;
        assert!(loss >= 0.0);

        // 4. TEST DE NETTOYAGE D'HISTORIQUE
        orch.session.add_user_message("Test");
        orch.clear_history().await?;
        assert_eq!(orch.session.history.len(), 0);

        Ok(())
    }

    /// 🎯 NOUVEAU TEST : Résilience face à un World Model corrompu sur disque
    #[async_test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_orchestrator_wm_resilience() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await?;

        let config = AppConfig::get();
        let manager = CollectionsManager::new(
            &sandbox.db,
            &config.mount_points.system.domain,
            &config.mount_points.system.db,
        );

        // Création d'un fichier Safetensors invalide (corrompu)
        let wm_dir = sandbox
            .db
            .config
            .db_root(
                &config.mount_points.system.domain,
                &config.mount_points.system.db,
            )
            .join("tensors/world_model");
        fs::ensure_dir_async(&wm_dir).await?;
        fs::write_async(wm_dir.join("world_model.safetensors"), b"CORRUPTED_DATA").await?;

        // L'orchestrateur doit détecter l'erreur, logger un Warning, et s'initialiser avec un modèle vierge
        let orch =
            AiOrchestrator::new(ProjectModel::default(), &manager, sandbox.db.clone()).await?;
        assert!(orch.world_engine.config.vocab_size > 0);

        Ok(())
    }
}
