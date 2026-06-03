// FICHIER : src-tauri/src/ai/context/memory_store.rs

use super::conversation_manager::ConversationSession;
use crate::json_db::collections::manager::CollectionsManager;
use crate::utils::prelude::*;

/// Gère la sauvegarde/chargement des sessions de chat via le Graphe de Connaissances (JSON-DB)
pub struct MemoryStore {
    pub collection_name: String,
}

impl MemoryStore {
    /// Initialise le store documentaire (collection `chat_sessions` ou définie dans les settings)
    pub async fn new(manager: &CollectionsManager<'_>) -> RaiseResult<Self> {
        // 🎯 GOUVERNANCE STRICTE : Vérification de l'activation du composant
        let _settings = match AppConfig::get_runtime_settings(
            manager,
            "ref:components:handle:ai_memory_store",
        )
        .await
        {
            Ok(s) => s,
            Err(e) => raise_error!(
                "ERR_MEMORY_STORE_INIT_REJECTED",
                error = e.to_string(),
                context = json_value!({"action": "memory_store_init", "hint": "Le composant ai_memory_store est-il actif et configuré dans le catalogue système ?"})
            ),
        };

        // 🎯 ZÉRO DETTE ABSOLUE : Aucun fallback codé en dur dans le binaire.
        // Si l'architecte système a oublié de configurer le nom de la collection, on crashe !
        let collection_name = match _settings.get("collection_name").and_then(|v| v.as_str()) {
            Some(name) => name.to_string(),
            None => raise_error!(
                "ERR_MEMORY_STORE_CONFIG_INVALID",
                error = "Le paramètre 'collection_name' est strictement requis mais absent de la configuration."
            ),
        };

        // Zéro fallback pour le schéma non plus.
        let schema_uri = match _settings.get("schema_uri").and_then(|v| v.as_str()) {
            Some(uri) => uri.to_string(),
            None => raise_error!(
                "ERR_MEMORY_STORE_CONFIG_INVALID",
                error = "Le paramètre 'schema_uri' est strictement requis mais absent de la configuration."
            ),
        };

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

    /// 🎯 HELPER ZÉRO DETTE : Injecte l'autorisation requise dans la base de données de test
    async fn inject_mock_memory_config(manager: &CollectionsManager<'_>) -> RaiseResult<()> {
        let config = AppConfig::get();
        let generic_schema_uri = format!(
            "db://{}/{}/schemas/v1/db/generic.schema.json",
            config.mount_points.system.domain, config.mount_points.system.db
        );

        // 🎯 FIX ZÉRO DETTE : On calcule l'URI attendue via les Mount Points système
        let session_schema_uri = format!(
            "db://{}/{}/schemas/v2/agents/memory/chat_session.schema.json",
            config.mount_points.system.domain, config.mount_points.system.db
        );

        // 1. CRÉATION DU COMPOSANT
        let _ = manager
            .create_collection("components", &generic_schema_uri)
            .await;
        manager
            .upsert_document(
                "components",
                json_value!({
                    "_id": "comp_memory_id",
                    "handle": "ai_memory_store",
                    "name": "Chat Memory Store"
                }),
            )
            .await?;

        // 2. CRÉATION DE LA CONFIGURATION
        let _ = manager
            .create_collection("service_configs", &generic_schema_uri)
            .await;
        manager
            .upsert_document(
                "service_configs",
                json_value!({
                    "_id": "mock_memory_cfg",
                    "component_id": "ref:components:handle:ai_memory_store",
                    "service_settings": {
                        "collection_name": "chat_sessions",
                        "schema_uri": session_schema_uri
                    }
                }),
            )
            .await?;

        Ok(())
    }

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

        // 🎯 FIX : Déblocage de la gouvernance pour memory_store
        inject_mock_memory_config(&manager).await?;

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
