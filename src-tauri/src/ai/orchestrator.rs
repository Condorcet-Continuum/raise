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
use crate::utils::{io::PathBuf, prelude::*, Arc};

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

    // Référence au StorageEngine pour les Agents
    storage: Option<Arc<StorageEngine>>,
}

impl AiOrchestrator {
    /// Constructeur
    pub async fn new(
        model: ProjectModel,
        qdrant_url: &str,
        llm_url: &str,
        storage_engine: Option<Arc<StorageEngine>>,
    ) -> Result<Self> {
        let app_config = AppConfig::get();
        let domain_path = app_config
            .get_path("PATH_RAISE_DOMAIN")
            .ok_or_else(|| AppError::Config("PATH_RAISE_DOMAIN manquant".into()))?;

        let base_path = domain_path;
        let chats_path = base_path.join("chats");
        let brain_path = base_path.join("world_model.safetensors");

        let rag = RagRetriever::new(qdrant_url, base_path.clone()).await?;
        let symbolic = SimpleRetriever::new(model);

        // Récupération des clés depuis la config fusionnée
        let gemini_key = app_config
            .ai_engines
            .get("cloud_gemini")
            .and_then(|e| e.api_key.clone())
            .unwrap_or_default();
        let model_name = app_config
            .ai_engines
            .get("primary_local")
            .map(|e| e.model_name.clone());

        let llm = LlmClient::new(llm_url, &gemini_key, model_name);

        let vocab_size = 10;
        let embedding_dim = 16;
        let action_dim = 5;
        let hidden_dim = 32;

        let world_engine = if brain_path.exists() {
            println!("🧠 [Orchestrator] Chargement du World Model...");
            NeuroSymbolicEngine::load_from_file(
                &brain_path,
                vocab_size,
                embedding_dim,
                action_dim,
                hidden_dim,
            )
            .await
            .unwrap_or_else(|e| {
                eprintln!("⚠️ Erreur chargement, création nouveau cerveau: {}", e);
                let vm = VarMap::new();
                NeuroSymbolicEngine::new(vocab_size, embedding_dim, action_dim, hidden_dim, vm)
                    .unwrap()
            })
        } else {
            println!("✨ [Orchestrator] Création d'un nouveau World Model vierge.");
            let vm = VarMap::new();
            NeuroSymbolicEngine::new(vocab_size, embedding_dim, action_dim, hidden_dim, vm)?
        };

        let memory_store = MemoryStore::new(&chats_path).await.map_err(|e| {
            AppError::Config(format!(
                "Impossible d'initialiser le stockage des chats: {}",
                e
            ))
        })?;

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
            storage: storage_engine,
        })
    }

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

    pub async fn execute_workflow(&mut self, user_query: &str) -> Result<AgentResult> {
        let _rag_context = self.rag.retrieve(user_query, 3).await.unwrap_or_default();

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

        let app_config = AppConfig::get(); //
        let domain_path = app_config
            .get_path("PATH_RAISE_DOMAIN")
            .expect("❌ [SSOT] PATH_RAISE_DOMAIN manquant"); //

        let dataset_path = app_config
            .get_path("PATH_RAISE_DATASET")
            .unwrap_or_else(|| domain_path.join("dataset")); //

        let storage_arc = self
            .storage
            .clone()
            .ok_or_else(|| AppError::Validation("StorageEngine manquant".into()))?;

        let mut hop_count = 0;
        const MAX_HOPS: i32 = 5;
        let mut accumulated_artifacts: Vec<CreatedArtifact> = Vec::new();
        let mut accumulated_messages: Vec<String> = Vec::new();

        loop {
            if hop_count >= MAX_HOPS {
                accumulated_messages.push("⚠️ Limite de redirections atteinte.".to_string());
                break;
            }

            let ctx = AgentContext::new(
                &current_agent_id,
                &global_session_id,
                storage_arc.clone(),
                self.llm.clone(),
                domain_path.clone(),
                dataset_path.clone(),
            );

            if let Some(agent) = self.create_agent(&current_agent_id) {
                let result_opt = agent.process(&ctx, &current_intent).await?;

                if let Some(res) = result_opt {
                    accumulated_artifacts.extend(res.artifacts);
                    accumulated_messages.push(res.message);

                    if let Some(acl_msg) = res.outgoing_message {
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
                break;
            }
        }

        Ok(AgentResult {
            message: accumulated_messages.join("\n\n---\n\n"),
            artifacts: accumulated_artifacts,
            outgoing_message: None,
        })
    }

    pub async fn ask(&mut self, query: &str) -> Result<String> {
        self.session.add_user_message(query);
        let prompt = format!("Expert Arcadia: {}", query);

        // ✅ Plus besoin de map_err, l'erreur est déjà du bon type !
        let response = self
            .llm
            .ask(LlmBackend::LocalLlama, "Tu es un expert.", &prompt)
            .await?;

        self.session.add_ai_message(&response);
        // Note: tu avais passé save_session en async précédemment,
        // n'oublie pas le .await si ce n'est pas déjà fait !
        let _ = self.memory_store.save_session(&self.session).await;
        Ok(response)
    }

    pub async fn reinforce_learning(
        &self,
        state_before: &ArcadiaElement,
        intent: CommandType,
        state_after: &ArcadiaElement,
    ) -> Result<f64> {
        let mut trainer = WorldTrainer::new(&self.world_engine, 0.01)
            .map_err(|e| AppError::Config(format!("Erreur Trainer: {}", e)))?;

        let loss = trainer
            .train_step(state_before, WorldAction { intent }, state_after)
            .map_err(|e| AppError::Validation(format!("Erreur TrainStep: {}", e)))?;

        let _ = self
            .world_engine
            .save_to_file(&self.world_engine_path)
            .await;
        Ok(loss)
    }

    pub async fn learn_document(&mut self, content: &str, source: &str) -> Result<usize> {
        self.rag
            .index_document(content, source)
            .await
            .map_err(|e| AppError::Validation(format!("Erreur d'indexation RAG : {}", e)))
        // ✅ Conversion explicite
    }

    pub async fn clear_history(&mut self) -> Result<()> {
        self.session = ConversationSession::new(self.session.id.clone());
        let _ = self.memory_store.save_session(&self.session).await;
        Ok(())
    }
}

// =========================================================================
// TESTS DE ROBUSTESSE
// =========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::protocols::acl::{AclMessage, Performative};
    use crate::model_engine::types::NameType;
    use crate::utils::{data::HashMap, io::tempdir, Once};

    static INIT_TEST: Once = Once::new();
    struct TestContext {
        _dir: tempfile::TempDir,
    }
    impl TestContext {
        fn new() -> (Self, PathBuf) {
            let dir = tempdir().expect("create temp dir");
            INIT_TEST.call_once(|| {
                if let Err(e) = AppConfig::init() {
                    panic!("Impossible d'initialiser AppConfig pour les tests: {}", e);
                }
            });
            let path = AppConfig::get().get_path("PATH_RAISE_DOMAIN").unwrap();
            (Self { _dir: dir }, path)
        }
    }

    fn make_element(id: &str) -> ArcadiaElement {
        // CORRECTION : Utilisation d'une URI valide pour le test d'apprentissage
        // Cela garantit que l'encodeur détecte correctement la couche (LA) et la catégorie (Function)
        ArcadiaElement {
            id: id.to_string(),
            name: NameType::default(),
            kind: "https://raise.io/ontology/arcadia/la#LogicalFunction".to_string(),
            description: None,
            properties: HashMap::new(),
        }
    }

    #[tokio::test]
    async fn test_orchestrator_robust_init() {
        let (_ctx, _path) = TestContext::new(); // On initialise la sandbox via TestContext

        // ✅ On appelle avec 4 arguments (le path est géré en interne via AppConfig::get())
        let orch = AiOrchestrator::new(
            ProjectModel::default(),
            "http://dummy",
            "http://dummy",
            None,
        )
        .await
        .expect("Init failed");

        assert_eq!(orch.session.id, "main_session");
    }

    #[tokio::test]
    async fn test_full_acl_path() {
        let (_ctx, _path) = TestContext::new();
        let msg = AclMessage::new(
            Performative::Request,
            "hardware",
            "quality_manager",
            "Verify",
        );
        assert_eq!(msg.receiver, "quality_manager");
    }

    #[tokio::test]
    async fn test_learning_cycle() {
        let (_ctx, _path) = TestContext::new();
        let orch = AiOrchestrator::new(
            ProjectModel::default(),
            "http://dummy",
            "http://dummy",
            None,
        )
        .await
        .unwrap();

        let loss = orch
            .reinforce_learning(&make_element("1"), CommandType::Create, &make_element("2"))
            .await;

        assert!(loss.is_ok(), "L'apprentissage a échoué : {:?}", loss.err());
    }
}
