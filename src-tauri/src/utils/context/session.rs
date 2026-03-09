// FICHIER : src-tauri/src/utils/context/session.rs

// 1. Dépendances Métier (Base de données locale)
use crate::json_db::collections::manager::CollectionsManager;
use crate::json_db::storage::StorageEngine;

// 2. Core : Concurrence, Horloge, Identifiants et Erreurs
use crate::utils::core::error::RaiseResult;
use crate::utils::core::{AsyncRwLock, SharedRef, UniqueId, UtcClock};

// 3. Data : Configuration, Collections et Typage JSON
use crate::utils::data::config::AppConfig;
use crate::utils::data::json::{self, json_value};
use crate::utils::data::UnorderedMap;

// 4. Data : Traits pour les Macros #[derive(...)]
// (On importe tes alias métier pour qu'ils soient reconnus par serde/Rust)
use crate::utils::data::{Deserializable, Serializable};

// --- MODÈLES DE DONNÉES ---
#[derive(Debug, Clone, Serializable, Deserializable, PartialEq)] // 🎯 Traits métiers
#[serde(rename_all = "snake_case")]
pub enum SessionStatus {
    Active,
    Idle,
    Expired,
    Revoked,
}

#[derive(Debug, Clone, Serializable, Deserializable, PartialEq)]
pub struct SessionContext {
    pub current_domain: String,
    pub current_db: String,
    pub active_dapp: String,
}

#[derive(Debug, Clone, Serializable, Deserializable, PartialEq)]
pub struct CrudPolicy {
    pub read: bool,
    pub write: bool,
    pub execute: bool,
}

#[derive(Debug, Clone, Serializable, Deserializable, PartialEq)]
pub struct Session {
    pub _id: String,
    pub created_at: String,
    pub updated_at: String,

    pub user_id: String,
    pub user_name: String,
    pub status: SessionStatus,
    pub expires_at: String,
    pub last_activity_at: String,
    pub context: SessionContext,

    #[serde(default = "fallback_cached_permissions")]
    pub cached_permissions: Option<UnorderedMap<String, CrudPolicy>>, // 🎯 Remplacé
}

fn fallback_cached_permissions() -> Option<UnorderedMap<String, CrudPolicy>> {
    None
}

// --- GESTIONNAIRE D'ÉTAT ---

#[derive(Clone)]
pub struct SessionManager {
    current_session: SharedRef<AsyncRwLock<Option<Session>>>,
    storage: SharedRef<StorageEngine>,
}

impl SessionManager {
    pub fn new(storage: SharedRef<StorageEngine>) -> Self {
        Self {
            current_session: SharedRef::new(AsyncRwLock::new(None)),
            storage,
        }
    }

    fn get_db_manager(&self) -> CollectionsManager<'_> {
        let config = AppConfig::get();
        CollectionsManager::new(&self.storage, &config.system_domain, &config.system_db)
    }

    pub async fn start_session(&self, username_or_id: &str) -> RaiseResult<Session> {
        let config = AppConfig::get();

        let ctx = SessionContext {
            current_domain: config.system_domain.clone(),
            current_db: config.system_db.clone(),
            active_dapp: config.active_dapp.clone(),
        };

        // 🎯 Remplacé (UniqueId)
        let is_uuid = UniqueId::parse_str(username_or_id).is_ok();
        let final_user_id = if is_uuid {
            username_or_id.to_string()
        } else {
            "00000000-0000-0000-0000-000000000000".to_string()
        };

        let now = UtcClock::now(); // 🎯 Remplacé
        let session = Session {
            _id: UniqueId::new_v4().to_string(),
            created_at: now.to_rfc3339(),
            updated_at: now.to_rfc3339(),
            user_id: final_user_id,
            user_name: username_or_id.to_string(),
            status: SessionStatus::Active,
            expires_at: (now + chrono::Duration::hours(8)).to_rfc3339(),
            last_activity_at: now.to_rfc3339(),
            context: ctx,
            cached_permissions: None,
        };

        // 🎯 Remplacé
        let doc = json::serialize_to_value(&session)?;
        let mgr = self.get_db_manager();

        let _ = mgr.init_db().await;
        mgr.insert_with_schema("sessions", doc).await?;

        let mut lock = self.current_session.write().await;
        *lock = Some(session.clone());

        Ok(session)
    }

    pub async fn get_current_session(&self) -> Option<Session> {
        let lock = self.current_session.read().await;
        lock.clone()
    }

    pub async fn touch(&self) -> RaiseResult<()> {
        let mut session_to_update = None;

        if let Some(session) = self.current_session.write().await.as_mut() {
            let now = UtcClock::now().to_rfc3339(); // 🎯 Remplacé
            session.last_activity_at = now.clone();
            session.updated_at = now;
            session_to_update = Some(session.clone());
        }

        if let Some(session) = session_to_update {
            let mgr = self.get_db_manager();
            let patch = json_value!({
                "last_activity_at": session.last_activity_at,
                "updated_at": session.updated_at
            });
            let _ = mgr.update_document("sessions", &session._id, patch).await;
        }

        Ok(())
    }

    pub async fn end_session(&self) -> RaiseResult<()> {
        let session_id_to_delete = {
            let mut lock = self.current_session.write().await;
            if let Some(session) = lock.take() {
                Some(session._id)
            } else {
                None
            }
        };

        if let Some(id) = session_id_to_delete {
            let mgr = self.get_db_manager();
            let _ = mgr.delete_document("sessions", &id).await;
        }

        Ok(())
    }
}

// --- TESTS UNITAIRES ---

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json_db::collections::manager::CollectionsManager;
    use crate::utils::testing::mock::AgentDbSandbox; // 🎯 Le mock officiel

    fn test_uuid() -> String {
        UniqueId::new_v4().to_string()
    }

    #[tokio::test]
    async fn test_session_manager_initial_state() {
        let sandbox = AgentDbSandbox::new().await;
        let manager = SessionManager::new(sandbox.db.clone());

        assert!(
            manager.get_current_session().await.is_none(),
            "La session doit être vide au démarrage"
        );
    }

    #[tokio::test]
    async fn test_start_session_populates_context_from_config() {
        let sandbox = AgentDbSandbox::new().await;
        let manager = SessionManager::new(sandbox.db.clone());
        let user_uuid = test_uuid();

        let session = manager.start_session(&user_uuid).await.unwrap();

        assert_eq!(session.user_id, user_uuid);
        assert_eq!(session.status, SessionStatus::Active);
        assert!(
            !session._id.is_empty(),
            "L'UUID de la session doit être généré"
        );

        assert_eq!(session.context.current_domain, sandbox.config.system_domain);
        assert_eq!(session.context.current_db, sandbox.config.system_db);
        assert_eq!(session.context.active_dapp, sandbox.config.active_dapp);
    }

    #[tokio::test]
    async fn test_start_session_persists_to_db() {
        let sandbox = AgentDbSandbox::new().await;
        let manager = SessionManager::new(sandbox.db.clone());
        let user_uuid = test_uuid();

        let session = manager.start_session(&user_uuid).await.unwrap();

        let db_mgr = CollectionsManager::new(
            &sandbox.db,
            &sandbox.config.system_domain,
            &sandbox.config.system_db,
        );

        let doc_opt = db_mgr.get_document("sessions", &session._id).await.unwrap();
        assert!(
            doc_opt.is_some(),
            "La session n'a pas été sauvegardée dans json_db"
        );

        let doc = doc_opt.unwrap();
        assert_eq!(doc["user_id"], user_uuid);
        assert_eq!(doc["status"], "active");
    }

    #[tokio::test]
    async fn test_session_touch_updates_db() {
        let sandbox = AgentDbSandbox::new().await;
        let manager = SessionManager::new(sandbox.db.clone());
        let user_uuid = test_uuid();
        let session = manager.start_session(&user_uuid).await.unwrap();

        let initial_activity =
            chrono::DateTime::parse_from_rfc3339(&session.last_activity_at).unwrap();

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        manager.touch().await.unwrap();

        let mem_session = manager.get_current_session().await.unwrap();
        let mem_activity =
            chrono::DateTime::parse_from_rfc3339(&mem_session.last_activity_at).unwrap();
        assert!(mem_activity > initial_activity);

        let db_mgr = CollectionsManager::new(
            &sandbox.db,
            &sandbox.config.system_domain,
            &sandbox.config.system_db,
        );
        let doc = db_mgr
            .get_document("sessions", &session._id)
            .await
            .unwrap()
            .unwrap();
        let db_activity =
            chrono::DateTime::parse_from_rfc3339(doc["last_activity_at"].as_str().unwrap())
                .unwrap();

        assert_eq!(
            mem_activity, db_activity,
            "La DB et la mémoire sont désynchronisées"
        );
    }

    #[tokio::test]
    async fn test_end_session_deletes_from_db() {
        let sandbox = AgentDbSandbox::new().await;
        let manager = SessionManager::new(sandbox.db.clone());
        let user_uuid = test_uuid();
        let session = manager.start_session(&user_uuid).await.unwrap();

        manager.end_session().await.unwrap();
        assert!(manager.get_current_session().await.is_none());

        let db_mgr = CollectionsManager::new(
            &sandbox.db,
            &sandbox.config.system_domain,
            &sandbox.config.system_db,
        );
        let doc_opt = db_mgr.get_document("sessions", &session._id).await.unwrap();
        assert!(
            doc_opt.is_none(),
            "La session aurait dû être supprimée physiquement"
        );
    }

    #[tokio::test]
    async fn test_concurrent_session_reads() {
        let sandbox = AgentDbSandbox::new().await;
        let manager = SessionManager::new(sandbox.db.clone());
        let user_uuid = test_uuid();
        manager.start_session(&user_uuid).await.unwrap();

        let mut tasks = vec![];
        for _ in 0..50 {
            let mgr_clone = manager.clone();
            tasks.push(tokio::spawn(async move {
                let session = mgr_clone.get_current_session().await;
                assert!(session.is_some());
            }));
        }

        for task in tasks {
            let _ = task.await;
        }
    }
}
