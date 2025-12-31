use crate::ai::context::{
    conversation_manager::ConversationSession, memory_store::MemoryStore, rag::RagRetriever,
    retriever::SimpleRetriever,
};
use crate::ai::llm::client::{LlmBackend, LlmClient};
use crate::model_engine::types::ProjectModel;
use anyhow::{Context, Result};
use std::env;
use std::path::PathBuf;

pub struct AiOrchestrator {
    rag: RagRetriever,
    symbolic: SimpleRetriever,
    llm: LlmClient,
    // Nouveaux composants Mémoire
    session: ConversationSession,
    memory_store: MemoryStore,
}

impl AiOrchestrator {
    /// Initialise l'orchestrateur.
    /// Charge automatiquement la session "default_session" (pour l'instant).
    pub async fn new(model: ProjectModel, qdrant_url: &str, llm_url: &str) -> Result<Self> {
        // 1. Init des moteurs de recherche
        let rag = RagRetriever::new(qdrant_url).await?;
        let symbolic = SimpleRetriever::new(model);
        let llm = LlmClient::new(llm_url, "", None);

        // 2. Init de la Persistance (Basée sur le .env)
        // On cherche le chemin de stockage défini dans le .env, ou on utilise un défaut
        let domain_path =
            env::var("PATH_RAISE_DOMAIN").unwrap_or_else(|_| ".raise_storage".to_string());

        let chats_path = PathBuf::from(domain_path).join("chats");
        let memory_store = MemoryStore::new(&chats_path)
            .context("Impossible d'initialiser le stockage des chats")?;

        // 3. Chargement de la session (Id fixe pour le moment : 'main_session')
        // Dans le futur, cet ID viendra de l'UI.
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

    /// Prépare le contexte complet : Historique + Modèle + RAG
    async fn prepare_prompt(&mut self, query: &str) -> Result<String> {
        // 1. Recherche RAG & Symbolique
        let rag_context = self.rag.retrieve(query, 3).await?;
        let symbolic_context = self.symbolic.retrieve_context(query);

        // 2. Récupération de l'historique conversationnel
        let history_context = self.session.to_context_string();

        // 3. Construction du Prompt Système Unique
        let mut prompt = String::from(
            "Tu es l'assistant intelligent de RAISE (Expert Système Arcadia).\n\
             Réponds à la question de l'ingénieur en utilisant le contexte ci-dessous.\n\
             Si l'utilisateur fait référence à 'ça', 'il' ou 'le', regarde l'HISTORIQUE.\n\n",
        );

        // Injection des blocs (seulement si non vides pour économiser des tokens)
        if !history_context.is_empty() {
            prompt.push_str(&history_context);
        }

        if !symbolic_context.is_empty() {
            prompt.push_str("### MODÈLE SYSTÈME (Vérité Terrain) ###\n");
            prompt.push_str(&symbolic_context);
            prompt.push_str("\n\n");
        }

        if !rag_context.is_empty() {
            prompt.push_str("### DOCUMENTATION (Connaissance RAG) ###\n");
            prompt.push_str(&rag_context);
            prompt.push_str("\n\n");
        }

        prompt.push_str("### NOUVELLE QUESTION ###\n");
        prompt.push_str(query);

        Ok(prompt)
    }

    /// La méthode principale : Traite la question, met à jour la mémoire et répond.
    pub async fn ask(&mut self, query: &str) -> Result<String> {
        // A. On ajoute la question à la mémoire court-terme
        self.session.add_user_message(query);

        // B. On prépare le prompt géant
        let prompt = self.prepare_prompt(query).await?;

        println!("🗣️ Envoi au LLM ({} chars)...", prompt.len());

        // C. Appel LLM
        let response = self
            .llm
            .ask(LlmBackend::LlamaCpp, "Tu es un expert.", &prompt)
            .await
            .map_err(|e| anyhow::anyhow!("Erreur LLM: {}", e))?;

        // D. On sauvegarde la réponse et on persiste sur disque
        self.session.add_ai_message(&response);
        self.memory_store.save_session(&self.session)?;

        Ok(response)
    }

    pub async fn learn_document(&mut self, content: &str, source: &str) -> Result<()> {
        self.rag.index_document(content, source).await
    }

    /// (Pour le debug) Réinitialise la conversation
    pub fn clear_history(&mut self) -> Result<()> {
        self.session = ConversationSession::new(self.session.id.clone());
        self.memory_store.save_session(&self.session)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model_engine::types::{ArcadiaElement, NameType, ProjectModel};
    use serde_json::json;
    use std::collections::HashMap;
    use std::env;
    use std::time::Duration;

    fn create_mock_model() -> ProjectModel {
        let mut model = ProjectModel::default();
        let drone = ArcadiaElement {
            id: "uuid-drone-123".to_string(),
            name: NameType::String("Drone de Livraison".to_string()),
            kind: "http://raise.io/ontology/oa#OperationalActor".to_string(),
            properties: HashMap::from([(
                "description".to_string(),
                json!("Acteur principal du système"),
            )]),
        };
        model.oa.actors.push(drone);
        model
    }

    #[tokio::test]
    #[ignore]
    async fn test_conversation_memory() {
        // 1. CONFIGURATION VIA .ENV
        dotenvy::dotenv().expect("❌ .env manquant");
        let llm_url = env::var("RAISE_LOCAL_URL").expect("RAISE_LOCAL_URL manquant");
        let qdrant_port = env::var("PORT_QDRANT_GRPC").expect("PORT_QDRANT_GRPC manquant");

        // On force 127.0.0.1 pour Qdrant URL
        let qdrant_url = format!("http://127.0.0.1:{}", qdrant_port);

        // Health check rapide
        let client = reqwest::Client::new();
        let health_url = format!("{}/health", llm_url.trim_end_matches('/'));
        if client
            .get(&health_url)
            .timeout(Duration::from_secs(2))
            .send()
            .await
            .is_err()
        {
            println!("⚠️ TEST IGNORÉ : LLM éteint.");
            return;
        }

        // 2. INIT
        let model = create_mock_model();
        let mut orchestrator = AiOrchestrator::new(model, &qdrant_url, &llm_url)
            .await
            .expect("Init failed");

        // On nettoie l'historique pour le test
        orchestrator.clear_history().unwrap();

        // 3. TOUR 1 : Injection d'information dans la conversation
        println!("💬 Tour 1 : Définition du sujet");
        let query1 = "Je travaille sur le projet secret 'Zeus'. C'est un satellite météo.";
        let rep1 = orchestrator.ask(query1).await.expect("Fail Turn 1");
        println!("🤖 IA: {}", rep1);

        // 4. TOUR 2 : Question contextuelle (Référence anaphorique)
        println!("💬 Tour 2 : Question mémoire");
        // Ici, l'IA ne peut répondre QUE si elle se souvient du Tour 1
        let query2 = "Quel est le but de ce projet secret ?";
        let rep2 = orchestrator.ask(query2).await.expect("Fail Turn 2");
        println!("🤖 IA: {}", rep2);

        // 5. VALIDATION
        let rep2_lower = rep2.to_lowercase();
        assert!(
            rep2_lower.contains("météo") || rep2_lower.contains("satellite"),
            "❌ L'IA a oublié le contexte de la conversation !"
        );

        println!("✅ SUCCÈS : L'Orchestrateur a de la mémoire !");
    }
}
