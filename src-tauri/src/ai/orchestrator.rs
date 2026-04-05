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

    // 🎯 Nouveau : On garde l'espace et la db pour recréer un Manager à la volée (ex: Reinforce Learning)
    pub space: String,
    pub db_name: String,
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

        let rag = RagRetriever::new(manager).await?;
        let symbolic = SimpleRetriever::new(model);
        let llm = LlmClient::new(manager).await?;

        let wm_config = app_config.world_model.clone();

        // 🎯 L'Orchestrateur délègue entièrement la gestion physique au World Model via le Manager !
        let world_engine = if NeuroSymbolicEngine::exists(manager).await {
            tracing::info!("🧠 [Orchestrator] Chargement du World Model depuis JSON-DB...");
            NeuroSymbolicEngine::load(manager, wm_config.clone())
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

        let memory_store = match MemoryStore::new(manager).await {
            Ok(ms) => ms,
            Err(e) => raise_error!(
                "ERR_CHAT_MEMORY_STORE_INIT",
                error = e,
                context = json_value!({
                    "component": "CHAT_SYSTEM"
                })
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

    /// Point d'entrée principal : Exécute une requête utilisateur via le système multi-agents
    pub async fn execute_workflow(&mut self, user_query: &str) -> RaiseResult<AgentResult> {
        let app_config = AppConfig::get();
        let Some(storage_arc) = self.storage.clone() else {
            raise_error!(
                "ERR_AGENT_STORAGE_MISSING",
                error = "StorageEngine requis pour l'exécution des agents",
                context = json_value!({
                    "component": "ORCHESTRATOR",
                    "action": "execute_workflow"
                })
            );
        };

        let manager = CollectionsManager::new(
            storage_arc.as_ref(),
            &app_config.system_domain,
            &app_config.system_db,
        );

        let _rag_context = self
            .rag
            .retrieve(&manager, user_query, 3)
            .await
            .unwrap_or_default();
        let _arcadia_context = self.symbolic.retrieve_context(user_query);

        let classifier = IntentClassifier::new(self.llm.clone());
        let mut current_intent = classifier.classify(user_query).await;
        let mut current_agent_urn = current_intent.recommended_agent_id().to_string();

        let session_scope = current_intent.default_session_scope();
        let global_session_id =
            AgentContext::generate_default_session_id("orchestrator", session_scope);

        let domain_path = app_config.get_path("PATH_RAISE_DOMAIN").unwrap();
        let dataset_path = app_config
            .get_path("PATH_RAISE_DATASET")
            .unwrap_or_else(|| domain_path.join("dataset"));

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

        let app_config = AppConfig::get();
        let Some(storage_arc) = &self.storage else {
            raise_error!("ERR_STORAGE_MISSING", error = "StorageEngine manquant");
        };

        let manager = CollectionsManager::new(
            storage_arc.as_ref(),
            &app_config.system_domain,
            &app_config.system_db,
        );

        let rag_ctx = self
            .rag
            .retrieve(&manager, query, 3)
            .await
            .unwrap_or_default();
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
        let _ = self
            .memory_store
            .save_session(&manager, &self.session)
            .await;

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

        // 🎯 L'Orchestrateur recrée un manager localement pour sauvegarder dans la bonne DB !
        if let Some(storage_arc) = &self.storage {
            let manager = CollectionsManager::new(storage_arc.as_ref(), &self.space, &self.db_name);
            let _ = self.world_engine.save(&manager).await;
        }

        Ok(loss)
    }

    pub async fn learn_document(&mut self, content: &str, source: &str) -> RaiseResult<usize> {
        let app_config = AppConfig::get();
        let Some(storage_arc) = &self.storage else {
            raise_error!(
                "ERR_STORAGE_MISSING",
                error = "StorageEngine manquant pour l'indexation RAG"
            );
        };
        let manager = CollectionsManager::new(
            storage_arc.as_ref(),
            &app_config.system_domain,
            &app_config.system_db,
        );

        self.rag.index_document(&manager, content, source).await
    }

    pub async fn clear_history(&mut self) -> RaiseResult<()> {
        self.session = ConversationSession::new(self.session.id.clone());

        let app_config = AppConfig::get();
        let Some(storage_arc) = &self.storage else {
            raise_error!("ERR_STORAGE_MISSING", error = "StorageEngine manquant");
        };
        let manager = CollectionsManager::new(
            storage_arc.as_ref(),
            &app_config.system_domain,
            &app_config.system_db,
        );

        let _ = self
            .memory_store
            .save_session(&manager, &self.session)
            .await;

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

        // 🎯 L'ASTUCE : On construit le chemin ABSOLU vers ton vrai dossier utilisateur
        let models_dir = dirs::home_dir()
            .unwrap_or_default()
            .join("raise_domain/_system/ai-assets/models");

        let light_model_path = models_dir
            .join("quew2-5-5b/qwen2.5-1.5b-instruct-q4_k_m.gguf")
            .to_string_lossy()
            .to_string();
        let light_tokenizer_path = models_dir
            .join("quew2-5-5b/tokenizer.json")
            .to_string_lossy()
            .to_string();

        // On injecte les chemins absolus pour contourner le /tmp de la Sandbox
        inject_mock_component(
            &manager,
            "llm",
            json_value!({
                "rust_model_file": light_model_path,
                "rust_tokenizer_file": light_tokenizer_path
            }),
        )
        .await;

        inject_mock_component(
            &manager,
            "nlp",
            json_value!({
                "model_name": "minilm",
                "rust_config_file": "config.json",
                "rust_tokenizer_file": "tokenizer.json",
                "rust_safetensors_file": "model.safetensors"
            }),
        )
        .await;

        sandbox
    }

    #[async_test]
    async fn test_full_acl_path() {
        // Ce test ne charge pas d'IA, on peut le laisser à part
        let msg = AclMessage::new(
            Performative::Request,
            "hardware",
            "quality_manager",
            "Verify",
        );
        assert_eq!(msg.receiver, "quality_manager");
    }

    // 🎯 FIX : On regroupe tous les tests de l'Orchestrateur en UN SEUL CYCLE
    #[async_test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_orchestrator_lifecycle() {
        let _guard = get_hf_lock().lock().await;
        let sandbox = setup_mock_orchestrator_env().await;
        let manager = CollectionsManager::new(
            &sandbox.db,
            &sandbox.config.system_domain,
            &sandbox.config.system_db,
        );

        // 1. TEST D'INITIALISATION
        let mut orch = AiOrchestrator::new(ProjectModel::default(), &manager, sandbox.db.clone())
            .await
            .expect("L'initialisation a échoué");
        assert_eq!(orch.session.id, "main_session");

        // 2. TEST DE L'APPRENTISSAGE RAG
        let content = "Raise est une plateforme incroyable combinant RAG et modèles formels.";
        let res = orch.learn_document(content, "documentation.txt").await;
        assert!(res.is_ok());
        assert!(res.unwrap() > 0);

        // 3. TEST DU CYCLE D'APPRENTISSAGE NEURO-SYMBOLIQUE (WORLD MODEL)
        let loss = orch
            .reinforce_learning(&make_element("1"), CommandType::Create, &make_element("2"))
            .await;
        assert!(loss.is_ok(), "L'apprentissage a échoué : {:?}", loss.err());

        // 4. TEST DE NETTOYAGE D'HISTORIQUE
        orch.session.add_user_message("Bonjour");
        orch.session.add_ai_message("Bonjour Humain");
        assert_eq!(orch.session.history.len(), 2);

        let clear_res = orch.clear_history().await;
        assert!(clear_res.is_ok());
        assert_eq!(orch.session.history.len(), 0);
    }
}
