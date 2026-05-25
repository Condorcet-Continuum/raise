// FICHIER : src-tauri/src/rules_engine/store.rs
use crate::utils::prelude::*; // 🎯 Façade Unique RAISE

use crate::json_db::collections::manager::CollectionsManager;
use crate::json_db::collections::manager::SystemIndexTx;
use crate::rules_engine::analyzer::Analyzer;
use crate::rules_engine::ast::Rule;

#[derive(Debug)]
pub struct RuleStore<'a> {
    db_manager: &'a CollectionsManager<'a>,
    dependency_cache: UnorderedMap<String, Vec<String>>,
    pub rules_cache: UnorderedMap<String, Rule>,
}

impl<'a> RuleStore<'a> {
    pub fn new(db_manager: &'a CollectionsManager<'a>) -> Self {
        Self {
            db_manager,
            dependency_cache: UnorderedMap::new(),
            rules_cache: UnorderedMap::new(),
        }
    }

    /// Initialise le store en chargeant les règles depuis la collection système (Async)
    pub async fn sync_from_db(&mut self) -> RaiseResult<()> {
        let stored_rules = match self.db_manager.list_all("_system_rules").await {
            Ok(list) => list,
            Err(e) => raise_error!("ERR_RULES_SYNC_FAILED", error = e.to_string()),
        };

        // 🎯 RÉSILIENCE : Swap Atomique "Zéro Dette"
        // On construit les nouveaux caches localement pour éviter un état vide en cas d'erreur
        let mut new_dependency_cache = UnorderedMap::new();
        let mut new_rules_cache = UnorderedMap::new();

        for rule_val in stored_rules {
            match json::deserialize_from_value::<Rule>(rule_val.clone()) {
                Ok(rule) => {
                    let col = rule_val
                        .get("_target_collection")
                        .and_then(|v| v.as_str())
                        .unwrap_or("*");

                    if let Err(e) = Self::index_rule_in_target_cache(
                        &mut new_dependency_cache,
                        &mut new_rules_cache,
                        col,
                        rule,
                    ) {
                        user_warn!(
                            "WRN_RULES_INDEX_SKIP",
                            json_value!({ "error": e.to_string() })
                        );
                    }
                }
                Err(e) => {
                    user_warn!(
                        "WRN_RULES_DESERIALIZATION_SKIP",
                        json_value!({ "error": e.to_string() })
                    );
                }
            }
        }

        // Échange atomique des pointeurs
        self.dependency_cache = new_dependency_cache;
        self.rules_cache = new_rules_cache;

        Ok(())
    }

    /// Enregistre une règle de manière idempotente (Async)
    pub async fn save_rule_document(&self, collection: &str, rule: Rule) -> RaiseResult<Rule> {
        if let Some(existing_rule) = self.rules_cache.get(&rule.handle) {
            if *existing_rule == rule {
                return Ok(existing_rule.clone());
            }
        }

        let mut doc = match json::serialize_to_value(&rule) {
            Ok(v) => v,
            Err(e) => {
                raise_error!(
                    "ERR_RULE_SERIALIZATION_FAILED",
                    error = e.to_string(),
                    context = json_value!({ "handle": rule.handle, "target": collection })
                )
            }
        };

        if let Some(obj) = doc.as_object_mut() {
            obj.insert("_target_collection".to_string(), json_value!(collection));
            obj.remove("_id"); // Retiré pour garantir l'exécution de x_compute (uuid_v4)
        }

        let saved_doc = match self
            .db_manager
            .insert_with_schema("_system_rules", doc)
            .await
        {
            Ok(v) => v,
            Err(e) => {
                raise_error!(
                    "ERR_RULE_DB_WRITE_FAILED",
                    error = e.to_string(),
                    context = json_value!({ "handle": rule.handle, "target": collection })
                )
            }
        };

        // On re-désérialise pour obtenir la version complète (avec l'UUID injecté par le schéma)
        let final_rule: Rule = match json::deserialize_from_value(saved_doc) {
            Ok(r) => r,
            Err(e) => {
                raise_error!(
                    "ERR_RULE_REHYDRATION_FAILED",
                    error = e.to_string(),
                    context = json_value!({ "handle": rule.handle })
                )
            }
        };

        Ok(final_rule)
    }

    /// ÉTAPE 2 (DDL) : Promeut la règle dans l'index via le Jeton et met à jour la RAM
    pub fn promote_rule_to_index(
        &mut self,
        collection: &str,
        rule: Rule,
        tx: &mut SystemIndexTx<'_>, // 🎯 LE JETON EXCLUSIF AU BON MOMENT
    ) -> RaiseResult<()> {
        let uuid = rule._id.as_ref().ok_or_else(|| {
            build_error!(
                "ERR_RULE_UUID_MISSING",
                error = "Impossible de promouvoir une règle sans _id technique",
                context = json_value!({ "handle": rule.handle })
            )
        })?;

        let rule_link = json_value!({
            "file": format!("db://{}/{}/collections/_system_rules/{}.json",
                            self.db_manager.space, self.db_manager.db, uuid),
            "active": true
        });

        if tx.document.get("rules").is_none() {
            if let Some(doc_obj) = tx.document.as_object_mut() {
                doc_obj.insert("rules".to_string(), json_value!({}));
            }
        }

        if let Some(rules_obj) = tx.document.get_mut("rules").and_then(|v| v.as_object_mut()) {
            rules_obj.insert(rule.handle.clone(), rule_link);
        }

        Self::index_rule_in_target_cache(
            &mut self.dependency_cache,
            &mut self.rules_cache,
            collection,
            rule,
        )?;

        Ok(())
    }

    /// Indexation interne agnostique (utilisée par l'init et le hot-reload)
    fn index_rule_in_target_cache(
        dep_cache: &mut UnorderedMap<String, Vec<String>>,
        rule_cache: &mut UnorderedMap<String, Rule>,
        collection: &str,
        rule: Rule,
    ) -> RaiseResult<()> {
        let deps = Analyzer::get_dependencies(&rule.expr, 50)?;

        for dep in deps {
            let key = format!("{}::{}", collection, dep);
            // On indexe par HANDLE [cite: 23]
            dep_cache.entry(key).or_default().push(rule.handle.clone());
        }
        rule_cache.insert(rule.handle.clone(), rule);
        Ok(())
    }
    pub fn cache_inline_rule(&mut self, collection: &str, rule: Rule) -> RaiseResult<()> {
        Self::index_rule_in_target_cache(
            &mut self.dependency_cache,
            &mut self.rules_cache,
            collection,
            rule,
        )
    }

    pub fn get_impacted_rules(
        &self,
        collection: &str,
        changed_fields: &UniqueSet<String>,
    ) -> Vec<Rule> {
        let mut impacted_handles = UniqueSet::new();
        for field in changed_fields {
            let key = format!("{}::{}", collection, field);
            if let Some(handles) = self.dependency_cache.get(&key) {
                for handle in handles {
                    impacted_handles.insert(handle);
                }
            }
        }
        impacted_handles
            .iter()
            .filter_map(|handle| self.rules_cache.get(*handle))
            .cloned()
            .collect()
    }

    pub fn get_rules_for_target(&self, target: &str) -> Vec<Rule> {
        self.rules_cache
            .values()
            .filter(|r| r.target == target)
            .cloned()
            .collect()
    }

    pub fn get_all_rules(&self) -> Vec<Rule> {
        self.rules_cache.values().cloned().collect()
    }
}

// =========================================================================
// TESTS UNITAIRES ET RÉSILIENCE
// =========================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use crate::json_db::collections::manager::CollectionsManager;
    use crate::rules_engine::ast::Expr;
    use crate::utils::testing::mock::insert_mock_db;
    use crate::utils::testing::AgentDbSandbox;

    #[async_test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_store_idempotency_and_retrieval() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await?;
        let config = AppConfig::get();
        let manager = CollectionsManager::new(
            &sandbox.db,
            &config.mount_points.system.domain,
            &config.mount_points.system.db,
        );
        let schema_uri = format!(
            "db://{}/{}/schemas/v1/db/generic.schema.json",
            manager.space, manager.db
        );
        manager
            .create_collection("_system_rules", &schema_uri)
            .await?;

        let mut store = RuleStore::new(&manager);

        let rule1 = Rule {
            _id: None,
            handle: "r1".into(),
            target: "user_age".into(),
            expr: Expr::Val(json_value!(1)),
            description: None,
            severity: None,
        };
        let rule2 = Rule {
            _id: None,
            handle: "r2".into(),
            target: "system_status".into(),
            expr: Expr::Val(json_value!(2)),
            description: None,
            severity: None,
        };

        // 🎯 1. Phase DML Asynchrone (Sans verrou global, évite le deadlock I/O)
        let finalized_1 = store.save_rule_document("users", rule1).await?;
        let finalized_2 = store.save_rule_document("systems", rule2).await?;

        // 🎯 2. Phase DDL Synchrone (Avec la Preuve de Verrou et le Jeton)
        {
            let lock = manager
                .storage
                .get_index_lock(&manager.space, &manager.db)?;
            let guard = lock.lock().await;
            let mut tx = manager.begin_system_tx(&guard).await?;

            store.promote_rule_to_index("users", finalized_1, &mut tx)?;
            store.promote_rule_to_index("systems", finalized_2, &mut tx)?;

            tx.commit().await?; // Validation finale sur disque
        }

        assert_eq!(store.get_rules_for_target("user_age").len(), 1);
        assert_eq!(store.get_all_rules().len(), 2);
        Ok(())
    }

    #[async_test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_store_mount_point_resilience() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await?;
        let manager = CollectionsManager::new(&sandbox.db, "ghost_partition", "void_db");
        let res = crate::rules_engine::initialize_rules_engine(&manager).await;
        match res {
            Err(crate::utils::core::error::AppError::Structured(err)) => {
                assert_eq!(err.code, "ERR_RULES_ENGINE_INIT_FAIL");
                Ok(())
            }
            _ => raise_error!(
                "ERR_TEST_FAILED",
                error = "Le moteur aurait dû lever ERR_RULES_ENGINE_INIT_FAIL"
            ),
        }
    }

    #[async_test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_store_deserialization_resilience() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await?;
        let config = AppConfig::get();
        let manager = CollectionsManager::new(
            &sandbox.db,
            &config.mount_points.system.domain,
            &config.mount_points.system.db,
        );
        let schema_uri = format!(
            "db://{}/{}/schemas/v1/db/generic.schema.json",
            manager.space, manager.db
        );
        manager
            .create_collection("_system_rules", &schema_uri)
            .await?;
        let doc_rules = &json_value!({ "_id": "bad_rule", "expr": { "not_an_expr": true } });
        insert_mock_db(&manager, "_system_rules", doc_rules).await?;

        let mut store = RuleStore::new(&manager);
        store.sync_from_db().await?;

        assert_eq!(
            store.rules_cache.len(),
            0,
            "La règle corrompue doit être ignorée par le sync."
        );
        Ok(())
    }
}
