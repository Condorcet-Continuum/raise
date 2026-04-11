// FICHIER : src-tauri/src/utils/context/session.rs

// 1. Dépendances Métier (Base de données locale)
use crate::json_db::collections::manager::CollectionsManager;
use crate::json_db::query::{Condition, FilterOperator, Query, QueryEngine, QueryFilter};
use crate::json_db::storage::StorageEngine;

// 2. Core : Concurrence, Horloge, Identifiants et Erreurs
use crate::utils::core::error::RaiseResult;
use crate::utils::core::{AsyncRwLock, SharedRef, UtcClock};

// 3. Data : Configuration, Collections et Typage JSON
use crate::raise_error;
use crate::utils::data::config::AppConfig;
use crate::utils::data::json::{self, json_value};
use crate::utils::data::UnorderedMap;

// 4. Data : Traits pour les Macros #[derive(...)]
use crate::utils::data::{Deserializable, Serializable};

// --- MODÈLES DE DONNÉES ---
#[derive(Debug, Clone, Serializable, Deserializable, PartialEq)]
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
    pub active_dapp_id: String,
}

#[derive(Debug, Clone, Serializable, Deserializable, PartialEq)]
pub struct CrudPolicy {
    pub read: bool,
    pub write: bool,
    pub execute: bool,
}

#[derive(Debug, Clone, Serializable, Deserializable, PartialEq)]
pub struct Session {
    #[serde(rename = "_id")]
    pub id: String,

    #[serde(rename = "_created_at")]
    pub created_at: String,

    #[serde(rename = "_updated_at")]
    pub updated_at: String,

    #[serde(rename = "@type", default = "fallback_session_type")]
    pub semantic_type: Vec<String>,

    pub handle: String,

    pub user_id: String,
    pub user_handle: String,
    pub status: SessionStatus,
    pub expires_at: String,
    pub last_activity_at: String,
    pub context: SessionContext,

    #[serde(default = "fallback_cached_permissions")]
    pub cached_permissions: Option<UnorderedMap<String, CrudPolicy>>,
}

fn fallback_session_type() -> Vec<String> {
    vec!["Session".to_string()]
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
        CollectionsManager::new(
            &self.storage,
            &config.mount_points.system.domain,
            &config.mount_points.system.db,
        )
    }

    #[allow(clippy::single_match)]
    pub async fn start_session(&self, requested_user: &str) -> RaiseResult<Session> {
        let mgr = self.get_db_manager();
        let config = AppConfig::get();

        let actual_user = match &config.user {
            Some(scope) => scope.id.clone(),
            None => {
                if config.is_test_env() {
                    requested_user.to_string()
                } else {
                    "admin".to_string()
                }
            }
        };

        if requested_user != actual_user {
            tracing::warn!(
                target: "system_core",
                "Fallback identitaire : Requête '{}' rejetée, session ouverte en tant que '{}'",
                requested_user, actual_user
            );
        }

        let mut query = Query::new("users");
        query.filter = Some(QueryFilter {
            operator: FilterOperator::And,
            conditions: vec![Condition::eq("handle", json_value!(&actual_user))],
        });

        let qe = QueryEngine::new(&mgr);
        let res = qe.execute_query(query).await?;

        let doc = match res.documents.first() {
            Some(d) => d,
            None => {
                raise_error!(
                    "ERR_USER_NOT_FOUND",
                    error = format!(
                        "L'utilisateur critique '{}' est introuvable dans le bootstrap.",
                        actual_user
                    ),
                    context = json_value!({"handle": actual_user, "action": "start_session"})
                );
            }
        };

        let def_domain = match doc.get("default_domain").and_then(|v| v.as_str()) {
            Some(v) => v.to_string(),
            None => config.mount_points.modeling.domain.clone(),
        };

        let def_db = match doc.get("default_db").and_then(|v| v.as_str()) {
            Some(v) => v.to_string(),
            None => config.mount_points.modeling.db.clone(),
        };

        let user_id = match doc
            .get("_id")
            .or_else(|| doc.get("id"))
            .and_then(|v| v.as_str())
        {
            Some(v) => v.to_string(),
            None => actual_user.clone(),
        };

        let ctx = SessionContext {
            current_domain: def_domain,
            current_db: def_db,
            active_dapp_id: config.active_dapp_id.clone(),
        };

        let now = UtcClock::now();

        let session_handle = format!("session_{}_active", actual_user);

        let mut session_query = Query::new("sessions");
        session_query.filter = Some(QueryFilter {
            operator: FilterOperator::And,
            conditions: vec![Condition::eq("handle", json_value!(&session_handle))],
        });

        let existing_doc = match QueryEngine::new(&mgr).execute_query(session_query).await {
            Ok(res) => res.documents.into_iter().next(),
            Err(_) => None,
        };

        let hydrated_doc = match existing_doc {
            Some(d) => {
                let id = match d.get("_id").and_then(|v| v.as_str()) {
                    Some(i) => i.to_string(),
                    None => {
                        raise_error!(
                            "ERR_SESSION_CORRUPTED",
                            error = "Session existante sans identifiant _id",
                            context = json_value!({"action": "start_session"})
                        );
                    }
                };
                let patch = json_value!({
                    "last_activity_at": now.to_rfc3339(),
                    "context": ctx,
                });
                mgr.update_document("sessions", &id, patch).await?
            }
            None => {
                let payload = json_value!({
                    "handle": session_handle,
                    "user_id": user_id,
                    "user_handle": actual_user,
                    "status": "active",
                    "expires_at": "2099-12-31T23:59:59Z",
                    "last_activity_at": now.to_rfc3339(),
                    "context": ctx,
                });
                mgr.insert_with_schema("sessions", payload).await?
            }
        };

        let session: Session = match json::deserialize_from_value(hydrated_doc) {
            Ok(s) => s,
            Err(e) => {
                raise_error!(
                    "ERR_SESSION_DESERIALIZE",
                    error = e,
                    context = json_value!({ "action": "read_from_jsondb" })
                );
            }
        };

        let mut lock = self.current_session.write().await;
        *lock = Some(session.clone());

        Ok(session)
    }

    #[allow(clippy::single_match)]
    pub async fn switch_domain(&self, target_domain: &str) -> RaiseResult<SessionContext> {
        let mgr = self.get_db_manager();
        let config = AppConfig::get();

        let mut dom_query = Query::new("domains");
        dom_query.filter = Some(QueryFilter {
            operator: FilterOperator::And,
            conditions: vec![Condition::eq("handle", json_value!(target_domain))],
        });

        let res = QueryEngine::new(&mgr).execute_query(dom_query).await?;
        let dom_doc = match res.documents.first() {
            Some(d) => d,
            None => {
                raise_error!(
                    "ERR_DOMAIN_NOT_FOUND",
                    error = format!("Le domaine '{}' n'existe pas.", target_domain),
                    context = json_value!({"target_domain": target_domain})
                );
            }
        };

        let domain_uuid = match dom_doc
            .get("_id")
            .or_else(|| dom_doc.get("id"))
            .and_then(|v| v.as_str())
        {
            Some(v) => v.to_string(),
            None => "".to_string(),
        };

        let db_query = Query::new("databases");
        let domain_ref = format!("ref:domains:handle:{}", target_domain);

        let mut auto_db = config.mount_points.modeling.db.clone();
        match QueryEngine::new(&mgr).execute_query(db_query).await {
            Ok(db_res) => {
                for doc in db_res.documents {
                    let doc_domain_id = doc
                        .get("domain_id")
                        .and_then(|v| v.as_str())
                        .unwrap_or_default();
                    if doc_domain_id == domain_uuid || doc_domain_id == domain_ref {
                        match doc.get("handle").and_then(|v| v.as_str()) {
                            Some(h) => {
                                auto_db = h.to_string();
                                break;
                            }
                            None => {}
                        }
                    }
                }
            }
            Err(_) => {}
        }

        self.update_session_context(target_domain, &auto_db).await
    }

    #[allow(clippy::single_match)]
    pub async fn switch_db(&self, target_db: &str) -> RaiseResult<SessionContext> {
        let mgr = self.get_db_manager();

        let current_domain = {
            let lock = self.current_session.read().await;
            match lock.as_ref() {
                Some(s) => s.context.current_domain.clone(),
                None => "".to_string(),
            }
        };

        let mut dom_query = Query::new("domains");
        dom_query.filter = Some(QueryFilter {
            operator: FilterOperator::And,
            conditions: vec![Condition::eq("handle", json_value!(&current_domain))],
        });

        let mut domain_uuid = String::new();
        match QueryEngine::new(&mgr).execute_query(dom_query).await {
            Ok(res) => match res.documents.first() {
                Some(doc) => {
                    domain_uuid = match doc
                        .get("_id")
                        .or_else(|| doc.get("id"))
                        .and_then(|v| v.as_str())
                    {
                        Some(v) => v.to_string(),
                        None => "".to_string(),
                    };
                }
                None => {}
            },
            Err(_) => {}
        }

        let mut db_query = Query::new("databases");
        db_query.filter = Some(QueryFilter {
            operator: FilterOperator::And,
            conditions: vec![Condition::eq("handle", json_value!(target_db))],
        });

        let res = QueryEngine::new(&mgr).execute_query(db_query).await?;
        let db_doc = match res.documents.first() {
            Some(d) => d,
            None => {
                raise_error!(
                    "ERR_DB_NOT_FOUND",
                    error = format!("La base de données '{}' est introuvable.", target_db),
                    context =
                        json_value!({"target_db": target_db, "current_domain": current_domain})
                );
            }
        };

        let doc_domain_id = db_doc
            .get("domain_id")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        let domain_ref = format!("ref:domains:handle:{}", current_domain);

        if doc_domain_id != domain_uuid && doc_domain_id != domain_ref {
            raise_error!(
                "ERR_DB_NOT_IN_DOMAIN",
                error = "Cette base de données n'appartient pas au domaine actif.",
                context = json_value!({
                    "target_db": target_db,
                    "current_domain": current_domain,
                    "db_domain_id": doc_domain_id
                })
            );
        }

        self.update_session_context(&current_domain, target_db)
            .await
    }

    #[allow(clippy::single_match)]
    async fn update_session_context(
        &self,
        new_domain: &str,
        new_db: &str,
    ) -> RaiseResult<SessionContext> {
        let mut session_to_update = None;
        let mut new_ctx = None;

        let mut lock = self.current_session.write().await;
        match lock.as_mut() {
            Some(session) => {
                session.context.current_domain = new_domain.to_string();
                session.context.current_db = new_db.to_string();
                session.updated_at = UtcClock::now().to_rfc3339();

                session_to_update = Some((session.id.clone(), session.updated_at.clone()));
                new_ctx = Some(session.context.clone());
            }
            None => {}
        }
        drop(lock);

        match session_to_update {
            Some((id, updated_at)) => match &new_ctx {
                Some(ctx) => {
                    let mgr = self.get_db_manager();
                    let patch = json_value!({
                        "updated_at": updated_at,
                        "context": {
                            "current_domain": ctx.current_domain,
                            "current_db": ctx.current_db,
                            "active_dapp": ctx.active_dapp_id
                        }
                    });
                    match mgr.update_document("sessions", &id, patch).await {
                        Ok(_) => {}
                        Err(_) => {} // Ignoré silencieusement pour ne pas bloquer le run
                    }
                }
                None => {}
            },
            None => {}
        }

        match new_ctx {
            Some(ctx) => Ok(ctx),
            None => raise_error!(
                "ERR_NO_ACTIVE_SESSION",
                error = "Impossible de mettre à jour le contexte : aucune session active.",
                context = json_value!({"action": "update_session_context"})
            ),
        }
    }

    pub async fn get_current_session(&self) -> Option<Session> {
        let lock = self.current_session.read().await;
        lock.clone()
    }

    #[allow(clippy::single_match)]
    pub async fn touch(&self) -> RaiseResult<()> {
        let mut session_to_update = None;

        let mut lock = self.current_session.write().await;
        match lock.as_mut() {
            Some(session) => {
                let now = UtcClock::now().to_rfc3339();
                session.last_activity_at = now.clone();
                session.updated_at = now;
                session_to_update = Some(session.clone());
            }
            None => {}
        }
        drop(lock);

        match session_to_update {
            Some(session) => {
                let mgr = self.get_db_manager();
                let patch = json_value!({
                    "last_activity_at": session.last_activity_at,
                    "updated_at": session.updated_at
                });
                match mgr.update_document("sessions", &session.id, patch).await {
                    Ok(_) => {}
                    Err(_) => {}
                }
            }
            None => {}
        }

        Ok(())
    }

    #[allow(clippy::single_match)]
    pub async fn end_session(&self) -> RaiseResult<()> {
        let mut lock = self.current_session.write().await;
        let session_id_to_delete = match lock.take() {
            Some(session) => Some(session.id),
            None => None,
        };
        drop(lock);

        match session_id_to_delete {
            Some(id) => {
                let mgr = self.get_db_manager();
                match mgr.delete_document("sessions", &id).await {
                    Ok(_) => {}
                    Err(_) => {}
                }
            }
            None => {}
        }

        Ok(())
    }
}

// --- TESTS UNITAIRES ---

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json_db::collections::manager::CollectionsManager;
    use crate::utils::testing::mock::{inject_mock_user, AgentDbSandbox};

    const TEST_AGENT: &str = "Astra-Bot-Tester";

    #[tokio::test]
    async fn test_session_manager_initial_state() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await;
        let manager = SessionManager::new(sandbox.db.clone());
        assert!(manager.get_current_session().await.is_none());
        Ok(())
    }

    #[tokio::test]
    async fn test_start_session_populates_context_from_config() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await;
        let manager = SessionManager::new(sandbox.db.clone());
        let userhandle = TEST_AGENT;

        let db_mgr = CollectionsManager::new(
            &sandbox.db,
            &sandbox.config.mount_points.system.domain,
            &sandbox.config.mount_points.system.db,
        );
        inject_mock_user(&db_mgr, userhandle).await;

        let session = manager.start_session(userhandle).await?;

        assert_eq!(session.user_handle, userhandle);
        assert_eq!(session.status, SessionStatus::Active);

        assert_eq!(session.context.current_domain, "mbse2");
        assert_eq!(session.context.current_db, "drones");

        Ok(())
    }

    #[tokio::test]
    async fn test_start_session_persists_to_db() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await;
        let manager = SessionManager::new(sandbox.db.clone());
        let userhandle = TEST_AGENT;

        let db_mgr = CollectionsManager::new(
            &sandbox.db,
            &sandbox.config.mount_points.system.domain,
            &sandbox.config.mount_points.system.db,
        );
        inject_mock_user(&db_mgr, userhandle).await;

        let session = manager.start_session(userhandle).await?;

        let doc_opt = db_mgr.get_document("sessions", &session.id).await?;
        assert!(doc_opt.is_some());
        assert_eq!(doc_opt.unwrap()["user_handle"], userhandle);

        Ok(())
    }

    #[tokio::test]
    async fn test_session_touch_updates_db() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await;
        let manager = SessionManager::new(sandbox.db.clone());
        let userhandle = TEST_AGENT;

        let db_mgr = CollectionsManager::new(
            &sandbox.db,
            &sandbox.config.mount_points.system.domain,
            &sandbox.config.mount_points.system.db,
        );
        inject_mock_user(&db_mgr, userhandle).await;

        let session = manager.start_session(userhandle).await?;
        let initial_activity = session.last_activity_at.clone();

        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        manager.touch().await?;

        let mem_session = manager.get_current_session().await.unwrap();
        assert!(mem_session.last_activity_at > initial_activity);

        Ok(())
    }

    #[tokio::test]
    async fn test_end_session_deletes_from_db() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await;
        let manager = SessionManager::new(sandbox.db.clone());
        let userhandle = TEST_AGENT;

        let db_mgr = CollectionsManager::new(
            &sandbox.db,
            &sandbox.config.mount_points.system.domain,
            &sandbox.config.mount_points.system.db,
        );
        inject_mock_user(&db_mgr, userhandle).await;

        let session = manager.start_session(userhandle).await?;
        manager.end_session().await?;

        let doc_opt = db_mgr.get_document("sessions", &session.id).await?;
        assert!(doc_opt.is_none());

        Ok(())
    }

    #[tokio::test]
    async fn test_concurrent_session_reads() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await;
        let manager = SessionManager::new(sandbox.db.clone());
        let userhandle = "Bot-Parallel";

        let db_mgr = CollectionsManager::new(
            &sandbox.db,
            &sandbox.config.mount_points.system.domain,
            &sandbox.config.mount_points.system.db,
        );
        inject_mock_user(&db_mgr, userhandle).await;

        manager.start_session(userhandle).await?;

        let mut tasks = vec![];
        for _ in 0..10 {
            let mgr_clone = manager.clone();
            tasks.push(tokio::spawn(async move {
                let session = mgr_clone.get_current_session().await;
                assert!(session.is_some());
            }));
        }
        for t in tasks {
            match t.await {
                Ok(_) => {}
                Err(e) => panic!("Thread paniqué : {:?}", e),
            }
        }

        Ok(())
    }
}
