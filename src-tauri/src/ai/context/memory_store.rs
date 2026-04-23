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
        let app_config = AppConfig::get();

        // 🎯 FIX MOUNT POINTS : Utilisation du domaine système pour le schéma de session
        let schema_uri = format!(
            "db://{}/{}/schemas/v2/agents/memory/chat_session.schema.json",
            app_config.mount_points.system.domain, app_config.mount_points.system.db
        );

        // Tentative de création de la collection (ignorée si elle existe déjà)
        if let Err(e) = manager
            .create_collection(&collection_name, &schema_uri)
            .await
        {
            user_warn!(
                "WRN_SESSION_COLLECTION_INIT",
                json_value!({ "collection": collection_name, "technical_error": e.to_string(), "hint": "Ignoré si la collection existe déjà" })
            );
        }

        Ok(Self { collection_name })
    }

    /// Sauvegarde une session
    pub async fn save_session(
        &self,
        manager: &CollectionsManager<'_>,
        session: &ConversationSession,
    ) -> RaiseResult<()> {
        let mut doc = match json::serialize_to_value(session) {
            Ok(d) => d,
            Err(e) => raise_error!(
                "ERR_SESSION_SERIALIZE",
                error = e,
                context = json_value!({"session_id": session.id})
            ),
        };

        // 🎯 Zéro Dette : On s'assure que _id est bien défini pour l'upsert
        if let Some(obj) = doc.as_object_mut() {
            obj.insert("_id".to_string(), json_value!(session.id.clone()));
        }

        match manager.upsert_document(&self.collection_name, doc).await {
            Ok(_) => Ok(()),
            Err(e) => raise_error!(
                "ERR_SESSION_UPSERT",
                error = e,
                context =
                    json_value!({"collection": self.collection_name, "session_id": session.id})
            ),
        }
    }

    /// Charge une session existante ou en crée une nouvelle si absente
    pub async fn load_or_create(
        &self,
        manager: &CollectionsManager<'_>,
        session_id: &str,
    ) -> RaiseResult<ConversationSession> {
        // 🎯 Pattern matching strict sur la récupération du document
        match manager
            .get_document(&self.collection_name, session_id)
            .await?
        {
            Some(doc) => match json::deserialize_from_value::<ConversationSession>(doc) {
                Ok(session) => Ok(session),
                Err(e) => {
                    user_warn!(
                        "ERR_SESSION_CORRUPTED",
                        json_value!({ "session_id": session_id, "technical_error": e.to_string() })
                    );
                    Ok(ConversationSession::new(session_id.to_string()))
                }
            },
            None => Ok(ConversationSession::new(session_id.to_string())),
        }
    }

    /// Liste toutes les sessions disponibles
    pub async fn list_sessions(
        &self,
        manager: &CollectionsManager<'_>,
    ) -> RaiseResult<Vec<String>> {
        let mut sessions = Vec::new();

        // 🎯 FIX : Interception propre
        let docs = match manager.list_all(&self.collection_name).await {
            Ok(d) => d,
            Err(e) => raise_error!(
                "ERR_SESSION_LIST",
                error = e,
                context = json_value!({"collection": self.collection_name})
            ),
        };

        for doc in docs {
            if let Some(id) = doc.get("_id").and_then(|v| v.as_str()) {
                sessions.push(id.to_string());
            }
        }

        Ok(sessions)
    }
}

// =========================================================================
// TESTS UNITAIRES
// =========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::testing::{AgentDbSandbox, DbSandbox};

    #[async_test]
    async fn test_memory_store_lifecycle() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await?;
        let config = AppConfig::get();

        // 🎯 FIX MOUNT POINTS : Initialisation via le point de montage système de la sandbox
        let manager = CollectionsManager::new(
            &sandbox.db,
            &config.mount_points.system.domain,
            &config.mount_points.system.db,
        );

        // 🎯 FIX : Utiliser le mock de la sandbox (v1) au lieu de l'init de prod (v2)
        DbSandbox::mock_db(&manager).await?;

        // 🎯 Match strict sur la création du store
        let store = match MemoryStore::new(&manager).await {
            Ok(s) => s,
            Err(e) => panic!("Échec init MemoryStore : {:?}", e),
        };

        let session_id = "test_session_123";

        // 1. Création d'une session vierge
        let mut session = store.load_or_create(&manager, session_id).await?;
        assert_eq!(session.id, session_id);
        assert!(session.history.is_empty());

        // 2. Modification et Sauvegarde documentaire
        session.add_user_message("Hello AI");
        session.add_ai_message("Hello Human");
        store.save_session(&manager, &session).await?;

        // 3. Rechargement
        let reloaded = store.load_or_create(&manager, session_id).await?;
        assert_eq!(reloaded.history.len(), 2);
        assert_eq!(reloaded.history[0].content, "Hello AI");

        // 4. Liste
        let sessions = store.list_sessions(&manager).await?;
        assert!(sessions.contains(&session_id.to_string()));

        Ok(())
    }
}
