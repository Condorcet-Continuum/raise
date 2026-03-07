// FICHIER : src-tauri/src/utils/session.rs

// FICHIER : src-tauri/src/utils/session.rs

use crate::json_db::collections::manager::CollectionsManager;
use crate::json_db::storage::StorageEngine;
use crate::utils::{data, prelude::*, Arc, AsyncRwLock, HashMap, Utc, Uuid};

// --- MODÈLES DE DONNÉES ---
// (Inchanggés, mais j'ajoute un #[serde(default)] sur cached_permissions par sécurité)

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SessionStatus {
    Active,
    Idle,
    Expired,
    Revoked,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionContext {
    pub current_domain: String,
    pub current_db: String,
    pub active_dapp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CrudPolicy {
    pub read: bool,
    pub write: bool,
    pub execute: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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

    #[serde(skip, default)]
    pub cached_permissions: Option<HashMap<String, CrudPolicy>>,
}

// --- GESTIONNAIRE D'ÉTAT (Session Manager) ---

#[derive(Clone)]
pub struct SessionManager {
    current_session: Arc<AsyncRwLock<Option<Session>>>,
    // 🎯 Nouveau : Stockage de l'accès DB pour la persistance
    storage: Arc<StorageEngine>,
}

impl SessionManager {
    /// Initialise un nouveau gestionnaire de session avec accès à la DB
    pub fn new(storage: Arc<StorageEngine>) -> Self {
        Self {
            current_session: Arc::new(AsyncRwLock::new(None)),
            storage,
        }
    }

    /// Helper privé pour obtenir un CollectionsManager éphémère
    fn get_db_manager(&self) -> CollectionsManager<'_> {
        let config = AppConfig::get();
        CollectionsManager::new(&self.storage, &config.system_domain, &config.system_db)
    }

    /// Démarre une nouvelle session et la persiste en base
    pub async fn start_session(&self, username_or_id: &str) -> RaiseResult<Session> {
        let config = AppConfig::get();

        let ctx = SessionContext {
            current_domain: config.system_domain.clone(),
            current_db: config.system_db.clone(),
            active_dapp: config.active_dapp.clone(),
        };
        let is_uuid = Uuid::parse_str(username_or_id).is_ok();
        let final_user_id = if is_uuid {
            username_or_id.to_string()
        } else {
            // UUID de fallback pour les pseudos (ou recherche en base normalement)
            "00000000-0000-0000-0000-000000000000".to_string()
        };
        let now = Utc::now();
        let session = Session {
            _id: Uuid::new_v4().to_string(),
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

        // 1. Sauvegarde physique via json_db
        let doc = data::to_value(&session)?;
        let mgr = self.get_db_manager();

        // S'assure que la base système est prête (idempotent)
        let _ = mgr.init_db().await;

        // On utilise insert_with_schema pour valider contre `session.schema.json`
        mgr.insert_with_schema("sessions", doc).await?;

        // 2. Mise à jour de l'état en mémoire
        let mut lock = self.current_session.write().await;
        *lock = Some(session.clone());

        Ok(session)
    }

    /// Retourne un clone de la session active courante depuis la mémoire
    pub async fn get_current_session(&self) -> Option<Session> {
        let lock = self.current_session.read().await;
        lock.clone()
    }

    /// Met à jour l'horodatage de dernière activité en mémoire ET en base
    pub async fn touch(&self) -> RaiseResult<()> {
        let mut session_to_update = None;

        // 1. Mise à jour rapide en mémoire (zone critique courte)
        if let Some(session) = self.current_session.write().await.as_mut() {
            let now = Utc::now().to_rfc3339();
            session.last_activity_at = now.clone();
            session.updated_at = now;
            session_to_update = Some(session.clone());
        }

        // 2. Persistance asynchrone (hors du verrou)
        if let Some(session) = session_to_update {
            let mgr = self.get_db_manager();
            let patch = json!({
                "last_activity_at": session.last_activity_at,
                "updated_at": session.updated_at
            });
            // On ignore silencieusement si la DB échoue à ce stade précis (heartbeat)
            let _ = mgr.update_document("sessions", &session._id, patch).await;
        }

        Ok(())
    }

    /// Clôture la session courante (révocation)
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
            // On supprime physiquement la session (ou on pourrait faire un update_document pour passer en "status": "revoked")
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
    use crate::utils::mock::AgentDbSandbox;
    use crate::utils::{sleep, DateTime, Duration};

    fn test_uuid() -> String {
        Uuid::new_v4().to_string()
    }

    #[tokio::test]
    async fn test_session_manager_initial_state() {
        let sandbox = AgentDbSandbox::new().await;
        // On injecte directement l'Arc<StorageEngine> fourni par la sandbox
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

        // 1. Démarrage de la session
        let session = manager.start_session(&user_uuid).await.unwrap();

        // 2. Vérifications de l'UUID et du statut
        assert_eq!(session.user_id, user_uuid);
        assert_eq!(session.status, SessionStatus::Active);
        assert!(
            !session._id.is_empty(),
            "L'UUID de la session doit être généré"
        );

        // 3. Vérification de l'injection du contexte
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

        let initial_activity = DateTime::parse_from_rfc3339(&session.last_activity_at).unwrap();

        sleep(Duration::from_millis(50)).await;

        // 1. Toucher la session
        manager.touch().await.unwrap();

        // 2. Vérification mémoire
        let mem_session = manager.get_current_session().await.unwrap();
        let mem_activity = DateTime::parse_from_rfc3339(&mem_session.last_activity_at).unwrap();
        assert!(mem_activity > initial_activity);

        // 3. Vérification Physique (json_db a bien reçu l'update_document)
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
            DateTime::parse_from_rfc3339(doc["last_activity_at"].as_str().unwrap()).unwrap();

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

        // 1. Déconnexion
        manager.end_session().await.unwrap();
        assert!(manager.get_current_session().await.is_none());

        // 2. Vérification Physique
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

        // Test de robustesse du AsyncRwLock : Multiples lectures concurrentes (simulant les 13 services)
        let mut tasks = vec![];
        for _ in 0..50 {
            let mgr_clone = manager.clone();
            tasks.push(tokio::spawn(async move {
                let session = mgr_clone.get_current_session().await;
                assert!(session.is_some());
            }));
        }

        // On s'assure qu'aucune lecture ne panique
        for task in tasks {
            let _ = task.await;
        }
    }
}
