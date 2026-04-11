// FICHIER : src-tauri/src/rules_engine/store.rs
use crate::utils::prelude::*; // 🎯 Façade Unique RAISE

use crate::json_db::collections::manager::CollectionsManager;
use crate::rules_engine::analyzer::Analyzer;
use crate::rules_engine::ast::Rule;

#[derive(Debug)]
pub struct RuleStore<'a> {
    /// Référence au gestionnaire de collections pour la persistance
    db_manager: &'a CollectionsManager<'a>,
    /// Cache en mémoire pour l'exécution rapide (index inversé)
    /// "collection::champ_modifié" -> Vec<"rule_id">
    dependency_cache: UnorderedMap<String, Vec<String>>,
    /// Cache des règles chargées : "rule_id" -> Rule
    pub rules_cache: UnorderedMap<String, Rule>,
}

impl<'a> RuleStore<'a> {
    /// Crée un nouveau store lié à un gestionnaire de base de données existant
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

        self.dependency_cache.clear();
        self.rules_cache.clear();

        for rule_val in stored_rules {
            match json::deserialize_from_value::<Rule>(rule_val.clone()) {
                Ok(rule) => {
                    // Extraction de la collection cible pour cloisonner le cache
                    let col = rule_val
                        .get("_target_collection")
                        .and_then(|v| v.as_str())
                        .unwrap_or("*");
                    self.index_rule_in_cache(col, rule);
                }
                Err(e) => {
                    user_warn!(
                        "WRN_RULES_DESERIALIZATION_SKIP",
                        json_value!({ "error": e.to_string() })
                    );
                    continue;
                }
            }
        }
        Ok(())
    }

    /// Enregistre une règle de manière idempotente (Async)
    pub async fn register_rule(&mut self, collection: &str, rule: Rule) -> RaiseResult<()> {
        // OPTIMISATION : Vérifier si la règle existe déjà et est identique
        if let Some(existing_rule) = self.rules_cache.get(&rule.id) {
            if *existing_rule == rule {
                return Ok(());
            }
        }

        // 1. Sauvegarde persistante via le manager de JSON-DB
        let mut doc = match json::serialize_to_value(&rule) {
            Ok(v) => v,
            Err(e) => raise_error!(
                "ERR_RULE_SERIALIZATION_FAILED",
                error = e.to_string(),
                context = json_value!({
                    "action": "serialize_rule_for_storage",
                    "rule_id": rule.id,
                    "target_collection": collection
                })
            ),
        };

        if let Some(obj) = doc.as_object_mut() {
            obj.insert("_target_collection".to_string(), json_value!(collection));
            obj.insert("_id".to_string(), json_value!(rule.id));
        }

        match self.db_manager.insert_raw("_system_rules", &doc).await {
            Ok(_) => (),
            Err(e) => raise_error!("ERR_RULE_DB_WRITE_FAILED", error = e.to_string()),
        }

        // 2. Mise à jour du cache mémoire (avec isolement par collection)
        self.index_rule_in_cache(collection, rule);
        Ok(())
    }

    /// Indexation interne dans le cache mémoire
    fn index_rule_in_cache(&mut self, collection: &str, rule: Rule) {
        let deps = Analyzer::get_dependencies(&rule.expr);
        for dep in deps {
            let key = format!("{}::{}", collection, dep);
            self.dependency_cache
                .entry(key)
                .or_default()
                .push(rule.id.clone());
        }
        self.rules_cache.insert(rule.id.clone(), rule);
    }

    /// Récupère les règles impactées par des changements (Mode Réactif)
    pub fn get_impacted_rules(
        &self,
        collection: &str,
        changed_fields: &UniqueSet<String>,
    ) -> Vec<Rule> {
        let mut impacted_ids = UniqueSet::new();

        for field in changed_fields {
            let key = format!("{}::{}", collection, field);
            if let Some(ids) = self.dependency_cache.get(&key) {
                for id in ids {
                    impacted_ids.insert(id);
                }
            }
        }

        impacted_ids
            .iter()
            .filter_map(|id| self.rules_cache.get(*id))
            .cloned()
            .collect()
    }

    /// Récupère toutes les règles ciblant une entité spécifique (Mode Validatif)
    pub fn get_rules_for_target(&self, target: &str) -> Vec<Rule> {
        self.rules_cache
            .values()
            .filter(|r| r.target == target)
            .cloned()
            .collect()
    }

    /// Récupère l'ensemble des règles connues (Dump)
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
    use crate::utils::testing::AgentDbSandbox;

    #[async_test]
    #[serial_test::serial] // 🎯 FIX : Protection CUDA (Sandbox active)
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_store_idempotency_and_retrieval() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await;
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
            id: "r1".into(),
            target: "user_age".into(),
            expr: Expr::Val(json_value!(1)),
            description: None,
            severity: None,
        };

        let rule2 = Rule {
            id: "r2".into(),
            target: "system_status".into(),
            expr: Expr::Val(json_value!(2)),
            description: None,
            severity: None,
        };

        store.register_rule("users", rule1.clone()).await?;
        store.register_rule("systems", rule2.clone()).await?;

        assert_eq!(store.get_rules_for_target("user_age").len(), 1);
        assert_eq!(store.get_all_rules().len(), 2);
        Ok(())
    }

    /// 🎯 NOUVEAU TEST : Résilience face à une base de données déconnectée
    #[async_test]
    #[serial_test::serial] // 🎯 FIX : Protection CUDA
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_store_mount_point_resilience() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await;
        // Manager pointant sur une partition invalide
        let manager = CollectionsManager::new(&sandbox.db, "ghost_partition", "void_db");

        // 🎯 FIX : Utilisation de la façade d'initialisation (Gatekeeper des Mount Points)
        let res = crate::rules_engine::initialize_rules_engine(&manager).await;

        // Le moteur doit intercepter l'échec d'I/O lors de la création forcée de la collection
        match res {
            Err(AppError::Structured(err)) => {
                // L'erreur levée par initialize_rules_engine est ERR_RULES_ENGINE_INIT_FAIL
                assert_eq!(err.code, "ERR_RULES_ENGINE_INIT_FAIL");
                Ok(())
            }
            _ => panic!(
                "Le moteur aurait dû lever ERR_RULES_ENGINE_INIT_FAIL via initialize_rules_engine"
            ),
        }
    }

    /// 🎯 NOUVEAU TEST : Robustesse face aux règles corrompues
    #[async_test]
    #[serial_test::serial] // 🎯 FIX : Protection CUDA
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_store_deserialization_resilience() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await;
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

        manager
            .insert_raw(
                "_system_rules",
                &json_value!({
                    "_id": "bad_rule",
                    "expr": { "not_an_expr": true }
                }),
            )
            .await?;

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
