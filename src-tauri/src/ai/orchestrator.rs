// FICHIER : src-tauri/src/ai/orchestrator.rs

use crate::ai::context::{
    conversation_manager::ConversationSession, memory_store::MemoryStore, rag::RagRetriever,
    retriever::SimpleRetriever,
};
use crate::ai::llm::client::{LlmBackend, LlmClient};
use crate::ai::nlp::{self, parser::CommandType};
// --- AJOUT : Import du Trainer ---
use crate::ai::world_model::{NeuroSymbolicEngine, WorldAction, WorldTrainer};
use crate::model_engine::types::{ArcadiaElement, ProjectModel}; // Besoin de ArcadiaElement pour le feedback
use candle_nn::VarMap;

use anyhow::{Context, Result};
use std::env;
use std::path::PathBuf;

pub struct AiOrchestrator {
    rag: RagRetriever,
    symbolic: SimpleRetriever,
    llm: LlmClient,
    session: ConversationSession,
    memory_store: MemoryStore,
    world_engine: NeuroSymbolicEngine,
    #[allow(dead_code)]
    world_engine_path: PathBuf,
}

impl AiOrchestrator {
    pub async fn new(model: ProjectModel, qdrant_url: &str, llm_url: &str) -> Result<Self> {
        // 1. DÉFINITION DES CHEMINS
        let domain_path =
            env::var("PATH_RAISE_DOMAIN").unwrap_or_else(|_| ".raise_storage".to_string());
        let base_path = PathBuf::from(&domain_path);
        let chats_path = base_path.join("chats");

        // Chemin de sauvegarde du cerveau
        let brain_path = base_path.join("world_model.safetensors");

        // 2. INIT MOTEURS
        let rag = RagRetriever::new(qdrant_url, base_path.clone()).await?;
        let symbolic = SimpleRetriever::new(model);
        let llm = LlmClient::new(llm_url, "", None);

        // --- WORLD MODEL : Initialisation ---
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

        // 3. INIT PERSISTANCE CHAT
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
        })
    }

    /// Prépare le prompt en agrégeant toutes les sources de contexte
    async fn prepare_prompt(
        &mut self,
        query: &str,
        simulation_result: Option<String>,
    ) -> Result<String> {
        // 1. RECHERCHE CONTEXTUELLE
        let rag_context = self.rag.retrieve(query, 3).await?;
        let symbolic_context = self.symbolic.retrieve_context(query);
        let history_context = self.session.to_context_string();

        // 2. CONSTRUCTION DU PROMPT
        let mut prompt = String::from(
            "Tu es l'assistant expert de RAISE (Ingénierie Système Arcadia).\n\
             Utilise le contexte ci-dessous pour répondre.\n\n",
        );

        if !history_context.is_empty() {
            prompt.push_str(&history_context);
        }

        // --- WORLD MODEL : Injection dans le Prompt ---
        if let Some(sim_text) = simulation_result {
            prompt.push_str("### SIMULATION COGNITIVE (Prédiction de l'IA) ###\n");
            prompt.push_str(&format!(
                "Si cette action est exécutée, le système prédit : {}\n\n",
                sim_text
            ));
        }

        if !symbolic_context.is_empty() {
            prompt.push_str("### MODÈLE SYSTÈME (Vérité Terrain) ###\n");
            prompt.push_str(&symbolic_context);
            prompt.push_str("\n\n");
        }

        if !rag_context.is_empty() {
            prompt.push_str("### DOCUMENTATION (RAG) ###\n");
            prompt.push_str(&rag_context);
            prompt.push_str("\n\n");
        }

        prompt.push_str("### QUESTION UTILISATEUR ###\n");
        prompt.push_str(query);

        // 3. SÉCURITÉ CONTEXT GUARD (NLP)
        let safe_prompt = nlp::tokenizers::truncate_tokens(&prompt, 3500);

        Ok(safe_prompt)
    }

    /// Point d'entrée principal : Router Intelligent
    pub async fn ask(&mut self, query: &str) -> Result<String> {
        // 1. DÉTECTION D'INTENTION RAPIDE (Fast Path)
        let intent = nlp::parser::simple_intent_detection(query);
        let mut simulation_info = None;

        if intent == CommandType::Delete || intent == CommandType::Create {
            println!(
                "⚡ [Fast Path] Commande détectée : {:?}. Lancement simulation...",
                intent
            );

            // --- WORLD MODEL : Simulation ---
            if let Some(root_element) = self.symbolic.get_root_element() {
                let action = WorldAction { intent };

                match self.world_engine.simulate(&root_element, action) {
                    Ok(predicted_tensor) => {
                        let val = predicted_tensor.mean_all()?.to_scalar::<f32>()?;
                        let sim_msg = format!("L'état du système va changer. Impact estimé (latent activation) : {:.4}", val);
                        simulation_info = Some(sim_msg);
                    }
                    Err(e) => eprintln!("❌ Erreur simulation : {}", e),
                }
            } else {
                println!("⚠️ Pas d'élément racine pour simuler l'action.");
            }
        }

        // 2. MEMOIRE COURT TERME
        self.session.add_user_message(query);

        // 3. PRÉPARATION DU PROMPT
        let prompt = self.prepare_prompt(query, simulation_info).await?;
        println!("🧠 [Orchestrator] Prompt Size: ~{} chars", prompt.len());

        // 4. INFÉRENCE LLM
        let response = self
            .llm
            .ask(LlmBackend::LlamaCpp, "Tu es un expert.", &prompt)
            .await
            .map_err(|e| anyhow::anyhow!("Erreur LLM: {}", e))?;

        // 5. PERSISTANCE
        self.session.add_ai_message(&response);
        self.memory_store.save_session(&self.session)?;

        Ok(response)
    }

    /// --- NOUVELLE MÉTHODE : APPRENTISSAGE PAR FEEDBACK ---
    /// Appelé quand une action a réellement été effectuée.
    /// Met à jour le cerveau pour que la prochaine prédiction soit meilleure.
    pub async fn reinforce_learning(
        &self,
        state_before: &ArcadiaElement,
        intent: CommandType,
        state_after: &ArcadiaElement,
    ) -> Result<f64> {
        println!("🎓 [Orchestrator] Apprentissage en cours...");

        // 1. Création temporaire du coach (Trainer)
        // Le Learning Rate est faible (0.01) pour un apprentissage stable
        let mut trainer = WorldTrainer::new(&self.world_engine, 0.01)?;
        let action = WorldAction { intent };

        // 2. Étape d'entraînement (Calcul erreur + Correction poids)
        let loss = trainer.train_step(state_before, action, state_after)?;

        // 3. Sauvegarde immédiate du cerveau amélioré
        self.world_engine
            .save_to_file(&self.world_engine_path)
            .await?;

        println!("✅ [Orchestrator] Cerveau mis à jour. Perte: {:.6}", loss);
        Ok(loss)
    }

    /// Apprend un document via le pipeline RAG optimisé
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
            }
            (Self { _temp_dir: dir }, path)
        }
    }

    // Helper pour créer des éléments fictifs pour le test d'apprentissage
    fn make_dummy_element(id: &str) -> ArcadiaElement {
        ArcadiaElement {
            id: id.to_string(),
            name: NameType::default(),
            kind: "https://arcadia/la#LogicalFunction".to_string(),
            description: None,
            properties: HashMap::new(),
        }
    }

    #[tokio::test]
    async fn test_orchestrator_init() {
        let (_ctx, _path) = TestContext::new();
        let model = ProjectModel::default();
        let orchestrator = AiOrchestrator::new(model, "http://dummy", "http://dummy").await;
        assert!(orchestrator.is_ok());
    }

    #[tokio::test]
    async fn test_orchestrator_learning() {
        let (_ctx, _path) = TestContext::new();
        let model = ProjectModel::default();
        let mut orch = AiOrchestrator::new(model, "http://dummy", "http://dummy")
            .await
            .unwrap();

        let doc = "La méthodologie Arcadia comporte 5 niveaux d'ingénierie.";
        let chunks = orch
            .learn_document(doc, "test_doc")
            .await
            .expect("Apprentissage échoué");
        assert_eq!(chunks, 1);
    }

    #[tokio::test]
    async fn test_orchestrator_history_management() {
        let (_ctx, _path) = TestContext::new();
        let model = ProjectModel::default();
        let mut orch = AiOrchestrator::new(model, "http://dummy", "http://dummy")
            .await
            .unwrap();
        let _ = orch.ask("Bonjour").await;
        let res = orch.clear_history();
        assert!(res.is_ok());
    }

    #[tokio::test]
    async fn test_orchestrator_full_prompt_flow() {
        let (_ctx, _path) = TestContext::new();
        let model = ProjectModel::default();
        let mut orch = AiOrchestrator::new(model, "http://dummy", "http://dummy")
            .await
            .unwrap();
        orch.learn_document("Raise", "info").await.unwrap();
        let res = orch.ask("Raise ?").await;
        match res {
            Ok(_) => panic!("Devrait échouer sans LLM"),
            Err(e) => assert!(
                e.to_string().contains("Erreur LLM")
                    || e.to_string().contains("client error")
                    || e.to_string().contains("connect")
            ),
        }
    }

    // --- NOUVEAU TEST : Apprentissage du Cerveau ---
    #[tokio::test]
    async fn test_orchestrator_reinforcement() {
        let (_ctx, _path) = TestContext::new();
        let model = ProjectModel::default();
        let orch = AiOrchestrator::new(model, "http://dummy", "http://dummy")
            .await
            .unwrap();

        // On simule une transition : Etat A -> Create -> Etat B
        let state_a = make_dummy_element("A");
        let state_b = make_dummy_element("B");

        // On appelle la méthode d'apprentissage
        let result = orch
            .reinforce_learning(&state_a, CommandType::Create, &state_b)
            .await;

        assert!(
            result.is_ok(),
            "L'apprentissage (et la sauvegarde) devrait réussir"
        );
        // On vérifie qu'on a bien une valeur de perte (loss) positive ou nulle
        assert!(result.unwrap() >= 0.0);
    }
}
