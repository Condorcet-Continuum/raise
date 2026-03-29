// FICHIER : src-tauri/src/ai/context/memory_store.rs

use super::conversation_manager::ConversationSession;
use crate::json_db::collections::manager::CollectionsManager;
use crate::utils::prelude::*;

/// Gère la sauvegarde/chargement des sessions de chat via le Graphe de Connaissances (JSON-DB)
pub struct MemoryStore {
    pub collection_name: String,
}

impl MemoryStore {
    /// Initialise le store documentaire (collection `chat_sessions`)
    pub async fn new(manager: &CollectionsManager<'_>) -> RaiseResult<Self> {
        let collection_name = "chat_sessions".to_string();

        // 🎯 Création automatique de la collection si elle n'existe pas
        let _ = manager
            .create_collection(
                &collection_name,
                "db://_system/_system/schemas/v1/db/generic.schema.json",
            )
            .await;

        Ok(Self { collection_name })
    }

    /// Sauvegarde une session
    pub async fn save_session(
        &self,
        manager: &CollectionsManager<'_>,
        session: &ConversationSession,
    ) -> RaiseResult<()> {
        let mut doc = json::serialize_to_value(session)?;

        // 🎯 On s'assure que _id est bien défini pour l'upsert
        doc["_id"] = json_value!(session.id.clone());

        manager.upsert_document(&self.collection_name, doc).await?;
        Ok(())
    }

    /// Charge une session existante ou en crée une nouvelle si absente
    pub async fn load_or_create(
        &self,
        manager: &CollectionsManager<'_>,
        session_id: &str,
    ) -> RaiseResult<ConversationSession> {
        if let Ok(Some(doc)) = manager
            .get_document(&self.collection_name, session_id)
            .await
        {
            if let Ok(session) = json::deserialize_from_value::<ConversationSession>(doc) {
                return Ok(session);
            }
        }
        Ok(ConversationSession::new(session_id.to_string()))
    }

    /// Liste toutes les sessions disponibles
    pub async fn list_sessions(
        &self,
        manager: &CollectionsManager<'_>,
    ) -> RaiseResult<Vec<String>> {
        let mut sessions = Vec::new();
        if let Ok(docs) = manager.list_all(&self.collection_name).await {
            for doc in docs {
                if let Some(id) = doc.get("_id").and_then(|v| v.as_str()) {
                    sessions.push(id.to_string());
                }
            }
        }
        Ok(sessions)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::testing::AgentDbSandbox;

    #[async_test]
    async fn test_memory_store_lifecycle() {
        let sandbox = AgentDbSandbox::new().await;
        let manager = CollectionsManager::new(
            &sandbox.db,
            &sandbox.config.system_domain,
            &sandbox.config.system_db,
        );
        manager.init_db().await.unwrap();

        let store = MemoryStore::new(&manager).await.unwrap();
        let session_id = "test_session_123";

        // 1. Création d'une session vierge
        let mut session = store.load_or_create(&manager, session_id).await.unwrap();
        assert_eq!(session.id, session_id);
        assert!(session.history.is_empty());

        // 2. Modification et Sauvegarde documentaire !
        session.add_user_message("Hello AI");
        session.add_ai_message("Hello Human");
        store.save_session(&manager, &session).await.unwrap();

        // 3. Rechargement
        let reloaded = store.load_or_create(&manager, session_id).await.unwrap();
        assert_eq!(reloaded.history.len(), 2);
        assert_eq!(reloaded.history[0].content, "Hello AI");

        // 4. Liste
        let sessions = store.list_sessions(&manager).await.unwrap();
        assert!(sessions.contains(&session_id.to_string()));
    }
}
