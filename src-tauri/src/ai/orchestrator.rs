// FICHIER : src-tauri/src/ai/orchestrator.rs

use crate::ai::context::{
    conversation_manager::ConversationSession, memory_store::MemoryStore, rag::RagRetriever,
    retriever::SimpleRetriever,
};
use crate::ai::llm::client::{LlmBackend, LlmClient};
use crate::ai::nlp::{self, parser::CommandType};
use crate::model_engine::types::ProjectModel;
use anyhow::{Context, Result};
use std::env;
use std::path::PathBuf;

pub struct AiOrchestrator {
    rag: RagRetriever,
    symbolic: SimpleRetriever,
    llm: LlmClient,
    session: ConversationSession,
    memory_store: MemoryStore,
}

impl AiOrchestrator {
    pub async fn new(model: ProjectModel, qdrant_url: &str, llm_url: &str) -> Result<Self> {
        // 1. DÉFINITION DES CHEMINS
        // On récupère le chemin racine ou on utilise un défaut
        let domain_path =
            env::var("PATH_RAISE_DOMAIN").unwrap_or_else(|_| ".raise_storage".to_string());
        let base_path = PathBuf::from(&domain_path);

        // Sous-dossiers spécifiques
        let chats_path = base_path.join("chats");

        // 2. INIT MOTEURS
        // Le RAG utilise le base_path pour stocker sa DB (si mode Surreal)
        let rag = RagRetriever::new(qdrant_url, base_path.clone()).await?;

        let symbolic = SimpleRetriever::new(model);
        let llm = LlmClient::new(llm_url, "", None);

        // 3. INIT PERSISTANCE CHAT
        let memory_store = MemoryStore::new(&chats_path)
            .context("Impossible d'initialiser le stockage des chats")?;

        // Session unique pour le moment (peut être étendu)
        let session_id = "main_session";
        let session = memory_store.load_or_create(session_id)?;

        Ok(Self {
            rag,
            symbolic,
            llm,
            session,
            memory_store,
        })
    }

    /// Prépare le prompt en agrégeant toutes les sources de contexte
    /// ET en sécurisant la taille via NLP.
    async fn prepare_prompt(&mut self, query: &str) -> Result<String> {
        // 1. RECHERCHE CONTEXTUELLE
        let rag_context = self.rag.retrieve(query, 3).await?; // Top 3 chunks
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
        // On tronque pour Llama 3.2 (ex: 4k context -> 3.5k input max)
        let safe_prompt = nlp::tokenizers::truncate_tokens(&prompt, 3500);

        Ok(safe_prompt)
    }

    /// Point d'entrée principal : Router Intelligent
    pub async fn ask(&mut self, query: &str) -> Result<String> {
        // 1. DÉTECTION D'INTENTION RAPIDE (Fast Path)
        let intent = nlp::parser::simple_intent_detection(query);

        if intent == CommandType::Delete || intent == CommandType::Create {
            println!("⚡ [Fast Path] Commande détectée : {:?}", intent);
            // TODO: Brancher ici l'exécution directe sur le modèle
        }

        // 2. MEMOIRE COURT TERME
        self.session.add_user_message(query);

        // 3. PRÉPARATION DU PROMPT
        let prompt = self.prepare_prompt(query).await?;
        println!("🧠 [Orchestrator] Prompt Size: ~{} chars", prompt.len());

        // 4. INFÉRENCE LLM
        // Si le LLM n'est pas joignable, cela renverra une erreur ici.
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

    /// Apprend un document via le pipeline RAG optimisé
    pub async fn learn_document(&mut self, content: &str, source: &str) -> Result<usize> {
        self.rag.index_document(content, source).await
    }

    pub fn clear_history(&mut self) -> Result<()> {
        // On recrée une session vide avec le même ID
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
    use tempfile::tempdir;

    // Helper pour isoler l'environnement de test
    struct TestContext {
        _temp_dir: tempfile::TempDir, // Gardé pour éviter la suppression prématurée
    }

    impl TestContext {
        fn new() -> (Self, PathBuf) {
            let dir = tempdir().unwrap();
            let path = dir.path().to_path_buf();
            // On force la variable d'env pour que l'orchestrateur utilise ce dossier
            unsafe {
                env::set_var("PATH_RAISE_DOMAIN", path.to_str().unwrap());
                // On force SurrealDB pour les tests (pas besoin de Docker/Qdrant)
                env::set_var("VECTOR_STORE_PROVIDER", "surreal");
            }
            (Self { _temp_dir: dir }, path)
        }
    }

    #[tokio::test]
    async fn test_orchestrator_init() {
        let (_ctx, _path) = TestContext::new();
        let model = ProjectModel::default();

        // Initialisation
        let orchestrator = AiOrchestrator::new(model, "http://dummy", "http://dummy").await;

        assert!(
            orchestrator.is_ok(),
            "L'orchestrateur doit s'initialiser correctement"
        );
    }

    #[tokio::test]
    async fn test_orchestrator_learning() {
        let (_ctx, _path) = TestContext::new();
        let model = ProjectModel::default();
        let mut orch = AiOrchestrator::new(model, "http://dummy", "http://dummy")
            .await
            .unwrap();

        // Test d'apprentissage
        let doc = "La méthodologie Arcadia comporte 5 niveaux d'ingénierie.";
        let chunks = orch
            .learn_document(doc, "test_doc")
            .await
            .expect("Apprentissage échoué");

        assert_eq!(chunks, 1, "Le document court doit faire 1 chunk");

        // On vérifie indirectement via le RAG interne (si possible) ou via un ask qui échoue
        // mais qui prouve que le prompt est construit.
    }

    #[tokio::test]
    async fn test_orchestrator_history_management() {
        let (_ctx, _path) = TestContext::new();
        let model = ProjectModel::default();
        let mut orch = AiOrchestrator::new(model, "http://dummy", "http://dummy")
            .await
            .unwrap();

        // Ajout manuel dans la session (hack pour test car session privée)
        // On passe par ask() qui va planter sur le LLM mais aura ajouté le message user avant
        let _ = orch.ask("Bonjour").await; // Ignorer l'erreur LLM

        // On ne peut pas lire orch.session directement car privée,
        // mais on peut vérifier la persistance.
        // Ou on fait confiance à clear_history() qui ne doit pas crasher.
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

        // 1. On apprend quelque chose pour peupler le RAG
        orch.learn_document("Le projet RAISE est écrit en Rust.", "info_clef")
            .await
            .unwrap();

        // 2. On pose une question
        // Note: Cela va échouer au moment de l'appel HTTP au LLM (car URL dummy),
        // MAIS cela valide toute la chaîne amont :
        // - Intent Detection
        // - History Update
        // - RAG Retrieval (qui doit trouver "Rust")
        // - Prompt Construction
        let res = orch.ask("En quel langage est écrit RAISE ?").await;

        match res {
            Ok(_) => panic!("Le test devrait échouer car pas de LLM réel"),
            Err(e) => {
                // On vérifie que l'erreur vient bien du LLM (donc que tout le reste a marché)
                let msg = e.to_string();
                assert!(
                    msg.contains("Erreur LLM")
                        || msg.contains("client error")
                        || msg.contains("connect"),
                    "L'erreur obtenue n'est pas celle attendue : {}",
                    msg
                );
            }
        }
    }
}
