// FICHIER : src-tauri/src/rules_engine/store.rs
use crate::utils::prelude::*;

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
        let stored_rules = self
            .db_manager
            .list_all("_system_rules")
            .await
            .unwrap_or_default();

        self.dependency_cache.clear();
        self.rules_cache.clear();

        for rule_val in stored_rules {
            if let Ok(rule) = json::deserialize_from_value::<Rule>(rule_val.clone()) {
                // Extraction de la collection cible pour cloisonner le cache
                let col = rule_val
                    .get("_target_collection")
                    .and_then(|v| v.as_str())
                    .unwrap_or("*");
                self.index_rule_in_cache(col, rule);
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

        self.db_manager.insert_raw("_system_rules", &doc).await?;

        // 2. Mise à jour du cache mémoire (avec isolement par collection)
        self.index_rule_in_cache(collection, rule);
        Ok(())
    }

    /// Indexation interne dans le cache mémoire
    fn index_rule_in_cache(&mut self, collection: &str, rule: Rule) {
        let deps = Analyzer::get_dependencies(&rule.expr);
        for dep in deps {
            // Création d'une clé isolée (ex: "drones::status")
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
            // Recherche de la clé isolée correspondante
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json_db::collections::manager::CollectionsManager;
    use crate::rules_engine::ast::Expr;
    use crate::utils::testing::AgentDbSandbox;

    #[async_test]
    async fn test_store_idempotency_and_retrieval() {
        let sandbox = AgentDbSandbox::new().await;
        let manager = CollectionsManager::new(
            &sandbox.db,
            &sandbox.config.system_domain,
            &sandbox.config.system_db,
        );

        manager
            .create_collection(
                "_system_rules",
                "db://_system/_system/schemas/v1/db/generic.schema.json",
            )
            .await
            .unwrap();

        let mut store = RuleStore::new(&manager);

        let rule1 = Rule {
            id: "r1".into(),
            target: "user_age".into(), // Cible spécifique
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

        // Enregistrement
        store.register_rule("users", rule1.clone()).await.unwrap();
        store.register_rule("systems", rule2.clone()).await.unwrap();

        // Test de récupération ciblée (get_rules_for_target)
        let user_rules = store.get_rules_for_target("user_age");
        assert_eq!(user_rules.len(), 1);
        assert_eq!(user_rules[0].id, "r1");

        let system_rules = store.get_rules_for_target("system_status");
        assert_eq!(system_rules.len(), 1);
        assert_eq!(system_rules[0].id, "r2");

        let unknown_rules = store.get_rules_for_target("unknown");
        assert!(unknown_rules.is_empty());

        // Test de récupération globale (get_all_rules)
        let all_rules = store.get_all_rules();
        assert_eq!(all_rules.len(), 2);
    }
}
