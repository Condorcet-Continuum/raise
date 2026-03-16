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
    #[serde(rename = "_id")]
    pub id: String,

    #[serde(rename = "_created_at")]
    pub created_at: String,

    #[serde(rename = "_updated_at")]
    pub updated_at: String,

    #[serde(rename = "@type", default = "fallback_session_type")]
    pub semantic_type: Vec<String>,

    pub user_id: String,
    pub user_handle: String,
    pub status: SessionStatus,
    pub expires_at: String,
    pub last_activity_at: String,
    pub context: SessionContext,

    #[serde(default = "fallback_cached_permissions")]
    pub cached_permissions: Option<UnorderedMap<String, CrudPolicy>>, // 🎯 Remplacé
}

fn fallback_session_type() -> Vec<String> {
    vec!["Session".to_string(), "cfg:Session".to_string()]
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

    pub async fn start_session(&self, target_user: &str) -> RaiseResult<Session> {
        let mgr = self.get_db_manager();
        let _ = mgr.init_db().await;
        let config = AppConfig::get();

        // 1. VÉRIFICATION DU USER DANS JSON_DB
        let mut query = Query::new("users");
        query.filter = Some(QueryFilter {
            operator: FilterOperator::And,
            conditions: vec![Condition::eq("handle", json_value!(target_user))],
        });

        let qe = QueryEngine::new(&mgr);
        let res = qe.execute_query(query).await?;

        let Some(doc) = res.documents.first() else {
            // 🎯 UTILISATION DE RAISE_ERROR!
            raise_error!(
                "ERR_USER_NOT_FOUND",
                error = format!(
                    "L'utilisateur '{}' est introuvable dans le système.",
                    target_user
                ),
                context = json_value!({"handle": target_user, "action": "start_session"})
            );
        };

        // 2. EXTRACTION DES PRÉFÉRENCES ET DE L'ID
        let def_domain = doc
            .get("default_domain")
            .and_then(|v| v.as_str())
            .unwrap_or(&config.system_domain)
            .to_string();
        let def_db = doc
            .get("default_db")
            .and_then(|v| v.as_str())
            .unwrap_or(&config.system_db)
            .to_string();
        let user_id = doc
            .get("_id")
            .or_else(|| doc.get("id"))
            .and_then(|v| v.as_str())
            .unwrap_or(target_user)
            .to_string();

        let ctx = SessionContext {
            current_domain: def_domain,
            current_db: def_db,
            active_dapp: config.active_dapp.clone(),
        };

        let now = UtcClock::now();
        let payload = json_value!({
            "user_id": user_id,
            "user_handle": target_user,
            "status": "active",
            "last_activity_at": now.to_rfc3339(),
            "context": ctx,
        });

        // 3. CRÉATION EN BASE
        let hydrated_doc = mgr.insert_with_schema("sessions", payload).await?;

        let session: Session = match json::deserialize_from_value(hydrated_doc) {
            Ok(s) => s,
            Err(e) => raise_error!(
                "ERR_SESSION_DESERIALIZE",
                error = e,
                context = json_value!({ "action": "read_from_jsondb" })
            ),
        };

        let mut lock = self.current_session.write().await;
        *lock = Some(session.clone());

        Ok(session)
    }

    pub async fn switch_domain(&self, target_domain: &str) -> RaiseResult<SessionContext> {
        let mgr = self.get_db_manager();
        let config = AppConfig::get();

        // 1. VÉRIFIER LE DOMAINE
        let mut dom_query = Query::new("domains");
        dom_query.filter = Some(QueryFilter {
            operator: FilterOperator::And,
            conditions: vec![Condition::eq("handle", json_value!(target_domain))],
        });

        let res = QueryEngine::new(&mgr).execute_query(dom_query).await?;
        let Some(dom_doc) = res.documents.first() else {
            // 🎯 UTILISATION DE RAISE_ERROR!
            raise_error!(
                "ERR_DOMAIN_NOT_FOUND",
                error = format!("Le domaine '{}' n'existe pas.", target_domain),
                context = json_value!({"target_domain": target_domain})
            );
        };

        let domain_uuid = dom_doc
            .get("_id")
            .or_else(|| dom_doc.get("id"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        // 2. TROUVER LA DB PAR DÉFAUT
        let db_query = Query::new("databases");
        let domain_ref = format!("ref:domains:handle:{}", target_domain);

        let mut auto_db = config.system_db.clone();
        if let Ok(db_res) = QueryEngine::new(&mgr).execute_query(db_query).await {
            for doc in db_res.documents {
                let doc_domain_id = doc.get("domain_id").and_then(|v| v.as_str()).unwrap_or("");
                if doc_domain_id == domain_uuid || doc_domain_id == domain_ref {
                    if let Some(h) = doc.get("handle").and_then(|v| v.as_str()) {
                        auto_db = h.to_string();
                        break;
                    }
                }
            }
        }

        // 3. METTRE À JOUR LA SESSION
        self.update_session_context(target_domain, &auto_db).await
    }

    pub async fn switch_db(&self, target_db: &str) -> RaiseResult<SessionContext> {
        let mgr = self.get_db_manager();

        let current_domain = {
            let lock = self.current_session.read().await;
            lock.as_ref()
                .map(|s| s.context.current_domain.clone())
                .unwrap_or_default()
        };

        // 1. RÉCUPÉRER L'UUID DU DOMAINE ACTIF
        let mut dom_query = Query::new("domains");
        dom_query.filter = Some(QueryFilter {
            operator: FilterOperator::And,
            conditions: vec![Condition::eq("handle", json_value!(&current_domain))],
        });

        let mut domain_uuid = String::new();
        if let Ok(res) = QueryEngine::new(&mgr).execute_query(dom_query).await {
            if let Some(doc) = res.documents.first() {
                domain_uuid = doc
                    .get("_id")
                    .or_else(|| doc.get("id"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
            }
        }

        // 2. VÉRIFIER L'APPARTENANCE
        let mut db_query = Query::new("databases");
        db_query.filter = Some(QueryFilter {
            operator: FilterOperator::And,
            conditions: vec![Condition::eq("handle", json_value!(target_db))],
        });

        let res = QueryEngine::new(&mgr).execute_query(db_query).await?;
        let Some(db_doc) = res.documents.first() else {
            // 🎯 UTILISATION DE RAISE_ERROR!
            raise_error!(
                "ERR_DB_NOT_FOUND",
                error = format!("La base de données '{}' est introuvable.", target_db),
                context = json_value!({"target_db": target_db, "current_domain": current_domain})
            );
        };

        let doc_domain_id = db_doc
            .get("domain_id")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let domain_ref = format!("ref:domains:handle:{}", current_domain);

        if doc_domain_id != domain_uuid && doc_domain_id != domain_ref {
            // 🎯 UTILISATION DE RAISE_ERROR!
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

        // 3. METTRE À JOUR LA SESSION
        self.update_session_context(&current_domain, target_db)
            .await
    }

    // Fonction utilitaire interne pour persister le contexte
    async fn update_session_context(
        &self,
        new_domain: &str,
        new_db: &str,
    ) -> RaiseResult<SessionContext> {
        let mut session_to_update = None;
        let mut new_ctx = None;

        if let Some(session) = self.current_session.write().await.as_mut() {
            session.context.current_domain = new_domain.to_string();
            session.context.current_db = new_db.to_string();
            session.updated_at = UtcClock::now().to_rfc3339();

            session_to_update = Some((session.id.clone(), session.updated_at.clone()));
            new_ctx = Some(session.context.clone());
        }

        if let Some((id, updated_at)) = session_to_update {
            if let Some(ctx) = &new_ctx {
                let mgr = self.get_db_manager();
                let patch = json_value!({
                    "updated_at": updated_at,
                    "context": {
                        "current_domain": ctx.current_domain,
                        "current_db": ctx.current_db,
                        "active_dapp": ctx.active_dapp
                    }
                });
                let _ = mgr.update_document("sessions", &id, patch).await;
            }
        }

        let Some(ctx) = new_ctx else {
            // 🎯 UTILISATION DE RAISE_ERROR!
            raise_error!(
                "ERR_NO_ACTIVE_SESSION",
                error = "Impossible de mettre à jour le contexte : aucune session active.",
                context = json_value!({"action": "update_session_context"})
            );
        };

        Ok(ctx)
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
            let _ = mgr.update_document("sessions", &session.id, patch).await;
        }

        Ok(())
    }

    pub async fn end_session(&self) -> RaiseResult<()> {
        let session_id_to_delete = {
            let mut lock = self.current_session.write().await;
            if let Some(session) = lock.take() {
                Some(session.id)
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
    use crate::utils::testing::mock::{inject_mock_user, AgentDbSandbox};

    // 🤖 Identité fixe pour l'agent de test
    const TEST_AGENT: &str = "Astra-Bot-Tester";

    #[tokio::test]
    async fn test_session_manager_initial_state() {
        let sandbox = AgentDbSandbox::new().await;
        let manager = SessionManager::new(sandbox.db.clone());
        assert!(manager.get_current_session().await.is_none());
    }

    #[tokio::test]
    async fn test_start_session_populates_context_from_config() {
        let sandbox = AgentDbSandbox::new().await;
        let manager = SessionManager::new(sandbox.db.clone());
        let userhandle = TEST_AGENT;

        let db_mgr = CollectionsManager::new(
            &sandbox.db,
            &sandbox.config.system_domain,
            &sandbox.config.system_db,
        );
        inject_mock_user(&db_mgr, userhandle).await;

        // 🎯 On se connecte avec le nom de l'agent injecté
        let session = manager.start_session(userhandle).await.unwrap();

        assert_eq!(session.user_handle, userhandle);
        assert_eq!(session.status, SessionStatus::Active);

        // 🎯 FIX ASSERTION : On vérifie que la surcharge utilisateur (mbse2) est appliquée
        // et non le défaut du système (_system).
        assert_eq!(session.context.current_domain, "mbse2");
        assert_eq!(session.context.current_db, "drones");
    }

    #[tokio::test]
    async fn test_start_session_persists_to_db() {
        let sandbox = AgentDbSandbox::new().await;
        let manager = SessionManager::new(sandbox.db.clone());
        let userhandle = TEST_AGENT;

        let db_mgr = CollectionsManager::new(
            &sandbox.db,
            &sandbox.config.system_domain,
            &sandbox.config.system_db,
        );
        inject_mock_user(&db_mgr, userhandle).await;

        let session = manager.start_session(userhandle).await.unwrap();

        let doc_opt = db_mgr.get_document("sessions", &session.id).await.unwrap();
        assert!(doc_opt.is_some());
        assert_eq!(doc_opt.unwrap()["user_handle"], userhandle);
    }

    #[tokio::test]
    async fn test_session_touch_updates_db() {
        let sandbox = AgentDbSandbox::new().await;
        let manager = SessionManager::new(sandbox.db.clone());
        let userhandle = TEST_AGENT;

        let db_mgr = CollectionsManager::new(
            &sandbox.db,
            &sandbox.config.system_domain,
            &sandbox.config.system_db,
        );
        inject_mock_user(&db_mgr, userhandle).await;

        let session = manager.start_session(userhandle).await.unwrap();
        let initial_activity = session.last_activity_at.clone();

        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        manager.touch().await.unwrap();

        let mem_session = manager.get_current_session().await.unwrap();
        assert!(mem_session.last_activity_at > initial_activity);
    }

    #[tokio::test]
    async fn test_end_session_deletes_from_db() {
        let sandbox = AgentDbSandbox::new().await;
        let manager = SessionManager::new(sandbox.db.clone());
        let userhandle = TEST_AGENT;

        let db_mgr = CollectionsManager::new(
            &sandbox.db,
            &sandbox.config.system_domain,
            &sandbox.config.system_db,
        );
        inject_mock_user(&db_mgr, userhandle).await;

        let session = manager.start_session(userhandle).await.unwrap();
        manager.end_session().await.unwrap();

        let doc_opt = db_mgr.get_document("sessions", &session.id).await.unwrap();
        assert!(doc_opt.is_none());
    }

    #[tokio::test]
    async fn test_concurrent_session_reads() {
        let sandbox = AgentDbSandbox::new().await;
        let manager = SessionManager::new(sandbox.db.clone());
        let userhandle = "Bot-Parallel";

        let db_mgr = CollectionsManager::new(
            &sandbox.db,
            &sandbox.config.system_domain,
            &sandbox.config.system_db,
        );
        inject_mock_user(&db_mgr, userhandle).await;

        manager.start_session(userhandle).await.unwrap();

        let mut tasks = vec![];
        for _ in 0..10 {
            let mgr_clone = manager.clone();
            tasks.push(tokio::spawn(async move {
                let session = mgr_clone.get_current_session().await;
                assert!(session.is_some());
            }));
        }
        for t in tasks {
            t.await.unwrap();
        }
    }
}
