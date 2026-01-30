// FICHIER : src-tauri/src/ai/orchestrator.rs

use crate::ai::context::{
    conversation_manager::ConversationSession, memory_store::MemoryStore, rag::RagRetriever,
    retriever::SimpleRetriever,
};
use crate::ai::llm::client::{LlmBackend, LlmClient};
use crate::ai::nlp::{self, parser::CommandType};
use crate::ai::world_model::{NeuroSymbolicEngine, WorldAction, WorldTrainer};
use crate::model_engine::types::{ArcadiaElement, ProjectModel};
use candle_nn::VarMap;

use crate::json_db::storage::StorageEngine;
use anyhow::{Context, Result};
use std::env;
use std::path::PathBuf;
use std::sync::Arc;

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
        // 1. DÉFINITION DES CHEMINS
        let domain_path =
            env::var("PATH_RAISE_DOMAIN").unwrap_or_else(|_| ".raise_storage".to_string());
        let base_path = PathBuf::from(&domain_path);
        let chats_path = base_path.join("chats");
        let brain_path = base_path.join("world_model.safetensors");

        // 2. INIT MOTEURS
        let rag = RagRetriever::new(qdrant_url, base_path.clone()).await?;
        let symbolic = SimpleRetriever::new(model);
        let gemini_key = env::var("RAISE_GEMINI_KEY").unwrap_or_default();
        let model_name = env::var("RAISE_MODEL_NAME").ok();
        let llm = LlmClient::new(llm_url, &gemini_key, model_name);

        // --- WORLD MODEL ---
        let vocab_size = 10;
        let embedding_dim = 15;
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

        // 3. PERSISTANCE CHAT
        let memory_store = MemoryStore::new(&chats_path)
            .context("Impossible d'initialiser le stockage des chats")?;
        let session_id = "main_session";
        let session = memory_store.load_or_create(session_id)?;

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

    /// Factory interne : Crée un agent à la volée
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

    /// MÉTHODE PRINCIPALE : Exécute le Workflow Multi-Agents unifié
    pub async fn execute_workflow(&mut self, user_query: &str) -> Result<AgentResult> {
        println!("🚀 [Orchestrator] Workflow démarré : '{}'", user_query);

        // 1. Enrichissement RAG (Disponible pour tous les agents)
        let rag_context = match self.rag.retrieve(user_query, 3).await {
            Ok(ctx) => ctx,
            Err(e) => {
                eprintln!("⚠️ RAG indisponible : {}", e);
                String::new()
            }
        };

        // 2. Classification de l'intention
        let classifier = IntentClassifier::new(self.llm.clone());
        let mut current_intent = classifier.classify(user_query).await;
        let mut current_agent_id = current_intent.recommended_agent_id().to_string();

        // 3. Mode Fallback (Legacy)
        if current_agent_id == "orchestrator_agent" {
            println!("💡 [Orchestrator] Mode Fallback (Legacy) activé.");
            let response = self.ask(user_query).await?;
            return Ok(AgentResult::text(response));
        }

        // 4. Initialisation du contexte Agent
        let session_scope = current_intent.default_session_scope();
        let global_session_id =
            AgentContext::generate_default_session_id("orchestrator", session_scope);

        let domain_path = env::var("PATH_RAISE_DOMAIN")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from(".raise_storage"));
        let dataset_path = domain_path.join("dataset");

        let storage_arc = self.storage.clone().ok_or_else(|| {
            anyhow::anyhow!("StorageEngine manquant dans l'Orchestrateur (requis pour Agents)")
        })?;

        // 5. BOUCLE DE RÉSOLUTION (ACL Loop)
        let mut hop_count = 0;
        const MAX_HOPS: i32 = 5;
        let mut accumulated_artifacts: Vec<CreatedArtifact> = Vec::new();
        let mut accumulated_messages: Vec<String> = Vec::new();

        loop {
            if hop_count >= MAX_HOPS {
                accumulated_messages
                    .push("⚠️ Arrêt forcé : Limite de redirections atteinte.".to_string());
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
                println!(
                    "🔄 [Hop {}] Exécution Agent: {}",
                    hop_count, current_agent_id
                );

                let result_opt = agent.process(&ctx, &current_intent).await?;

                if let Some(res) = result_opt {
                    accumulated_artifacts.extend(res.artifacts);
                    accumulated_messages.push(res.message);

                    if let Some(acl_msg) = res.outgoing_message {
                        println!("📨 Message ACL détecté vers : {}", acl_msg.receiver);
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
                if hop_count == 0 {
                    let response = self.ask(user_query).await?;
                    return Ok(AgentResult::text(response));
                }
                break;
            }
        }

        let final_message = accumulated_messages.join("\n\n---\n\n");
        let final_message_with_context = if !rag_context.is_empty() {
            format!("{}\n\n*(Contexte documentaire utilisé)*", final_message)
        } else {
            final_message
        };

        Ok(AgentResult {
            message: final_message_with_context,
            artifacts: accumulated_artifacts,
            outgoing_message: None,
        })
    }

    // --- MÉTHODES LEGACY & APPRENTISSAGE ---

    async fn prepare_prompt(
        &mut self,
        query: &str,
        simulation_result: Option<String>,
    ) -> Result<String> {
        let rag_context = self.rag.retrieve(query, 3).await?;
        let symbolic_context = self.symbolic.retrieve_context(query);
        let history_context = self.session.to_context_string();

        let mut prompt = String::from("Tu es l'assistant expert de RAISE (Ingénierie Système Arcadia).\nUtilise le contexte ci-dessous pour répondre.\n\n");
        if !history_context.is_empty() {
            prompt.push_str(&history_context);
        }
        if let Some(sim_text) = simulation_result {
            prompt.push_str("### SIMULATION COGNITIVE ###\n");
            prompt.push_str(&format!("Prédiction : {}\n\n", sim_text));
        }
        if !symbolic_context.is_empty() {
            prompt.push_str("### MODÈLE SYSTÈME ###\n");
            prompt.push_str(&symbolic_context);
            prompt.push_str("\n\n");
        }
        if !rag_context.is_empty() {
            prompt.push_str("### DOCUMENTATION (RAG) ###\n");
            prompt.push_str(&rag_context);
            prompt.push_str("\n\n");
        }
        prompt.push_str("### QUESTION ###\n");
        prompt.push_str(query);

        Ok(nlp::tokenizers::truncate_tokens(&prompt, 3500))
    }

    pub async fn ask(&mut self, query: &str) -> Result<String> {
        let intent = nlp::parser::simple_intent_detection(query);
        let mut simulation_info = None;

        if intent == CommandType::Delete || intent == CommandType::Create {
            println!("⚡ [Fast Path] Commande détectée : {:?}.", intent);
            if let Some(root_element) = self.symbolic.get_root_element() {
                let action = WorldAction { intent };
                if let Ok(predicted_tensor) = self.world_engine.simulate(&root_element, action) {
                    if let Ok(val) = predicted_tensor
                        .mean_all()
                        .and_then(|t| t.to_scalar::<f32>())
                    {
                        simulation_info = Some(format!("Impact estimé : {:.4}", val));
                    }
                }
            }
        }

        self.session.add_user_message(query);
        let prompt = self.prepare_prompt(query, simulation_info).await?;
        let response = self
            .llm
            .ask(LlmBackend::LlamaCpp, "Tu es un expert.", &prompt)
            .await
            .map_err(|e| anyhow::anyhow!("Erreur LLM: {}", e))?;

        self.session.add_ai_message(&response);
        self.memory_store.save_session(&self.session)?;

        Ok(response)
    }

    pub async fn reinforce_learning(
        &self,
        state_before: &ArcadiaElement,
        intent: CommandType,
        state_after: &ArcadiaElement,
    ) -> Result<f64> {
        let mut trainer = WorldTrainer::new(&self.world_engine, 0.01)?;
        let action = WorldAction { intent };
        let loss = trainer.train_step(state_before, action, state_after)?;
        self.world_engine
            .save_to_file(&self.world_engine_path)
            .await?;
        Ok(loss)
    }

    pub async fn learn_document(&mut self, content: &str, source: &str) -> Result<usize> {
        self.rag.index_document(content, source).await
    }

    pub fn clear_history(&mut self) -> Result<()> {
        self.session = ConversationSession::new(self.session.id.clone());
        self.memory_store.save_session(&self.session)?;
        Ok(())
    }
}

// =========================================================================
// TESTS UNITAIRES
// =========================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use crate::model_engine::types::NameType;
    use std::collections::HashMap;
    use tempfile::tempdir;

    struct TestContext {
        _temp_dir: tempfile::TempDir,
    }

    impl TestContext {
        fn new() -> (Self, PathBuf) {
            let dir = tempdir().unwrap();
            let path = dir.path().to_path_buf();
            unsafe {
                env::set_var("PATH_RAISE_DOMAIN", path.to_str().unwrap());
                env::set_var("VECTOR_STORE_PROVIDER", "surreal");
                env::set_var("RAISE_GEMINI_KEY", "dummy");
            }
            (Self { _temp_dir: dir }, path)
        }
    }

    fn make_dummy_element(id: &str) -> ArcadiaElement {
        ArcadiaElement {
            id: id.to_string(),
            name: NameType::default(),
            kind: "LogicalFunction".to_string(),
            description: None,
            properties: HashMap::new(),
        }
    }

    #[tokio::test]
    async fn test_orchestrator_init() {
        let (_ctx, _path) = TestContext::new();
        let model = ProjectModel::default();
        let orchestrator = AiOrchestrator::new(model, "http://dummy", "http://dummy", None).await;
        assert!(orchestrator.is_ok());
    }

    #[tokio::test]
    async fn test_orchestrator_learning() {
        let (_ctx, _path) = TestContext::new();
        let model = ProjectModel::default();
        let mut orch = AiOrchestrator::new(model, "http://dummy", "http://dummy", None)
            .await
            .unwrap();
        let doc = "Arcadia est une méthode.";
        let chunks = orch
            .learn_document(doc, "test")
            .await
            .expect("Apprentissage échoué");
        assert_eq!(chunks, 1);
    }

    #[tokio::test]
    async fn test_workflow_execution_dummy() {
        let (_ctx, _path) = TestContext::new();
        let model = ProjectModel::default();
        let mut orch = AiOrchestrator::new(model, "http://dummy", "http://dummy", None)
            .await
            .unwrap();
        let res = orch.execute_workflow("Crée un composant").await;
        assert!(res.is_err()); // Car Storage manquant
    }

    // TEST RESTAURÉ : Vérifie l'apprentissage par renforcement
    #[tokio::test]
    async fn test_orchestrator_reinforcement() {
        let (_ctx, _path) = TestContext::new();
        let model = ProjectModel::default();
        // Init avec None pour le storage (non requis pour le World Model pur)
        let orch = AiOrchestrator::new(model, "http://dummy", "http://dummy", None)
            .await
            .unwrap();

        let state_a = make_dummy_element("A");
        let state_b = make_dummy_element("B");

        let result = orch
            .reinforce_learning(&state_a, CommandType::Create, &state_b)
            .await;

        assert!(
            result.is_ok(),
            "L'apprentissage (et la sauvegarde) devrait réussir"
        );
        assert!(result.unwrap() >= 0.0);
    }
}
