// FICHIER : src-tauri/src/rules_engine/store.rs

use crate::json_db::collections::manager::CollectionsManager;
use crate::rules_engine::analyzer::Analyzer;
use crate::rules_engine::ast::Rule;
use anyhow::Result;
use serde_json::json;
use std::collections::{HashMap, HashSet};

#[derive(Debug)]
pub struct RuleStore<'a> {
    /// Référence au gestionnaire de collections pour la persistance
    db_manager: &'a CollectionsManager<'a>,
    /// Cache en mémoire pour l'exécution rapide (index inversé)
    /// "champ_modifié" -> Vec<"rule_id">
    dependency_cache: HashMap<String, Vec<String>>,
    /// Cache des règles chargées : "rule_id" -> Rule
    pub rules_cache: HashMap<String, Rule>,
}

impl<'a> RuleStore<'a> {
    /// Crée un nouveau store lié à un gestionnaire de base de données existant
    pub fn new(db_manager: &'a CollectionsManager<'a>) -> Self {
        Self {
            db_manager,
            dependency_cache: HashMap::new(),
            rules_cache: HashMap::new(),
        }
    }

    /// Initialise le store en chargeant les règles depuis la collection système (Async)
    pub async fn sync_from_db(&mut self) -> Result<()> {
        let stored_rules = self
            .db_manager
            .list_all("_system_rules")
            .await
            .unwrap_or_default();

        self.dependency_cache.clear();
        self.rules_cache.clear();

        for rule_val in stored_rules {
            if let Ok(rule) = serde_json::from_value::<Rule>(rule_val) {
                self.index_rule_in_cache(rule);
            }
        }
        Ok(())
    }

    /// Enregistre une règle de manière idempotente (Async)
    pub async fn register_rule(&mut self, collection: &str, rule: Rule) -> Result<()> {
        // OPTIMISATION : Vérifier si la règle existe déjà et est identique
        if let Some(existing_rule) = self.rules_cache.get(&rule.id) {
            if *existing_rule == rule {
                return Ok(());
            }
        }

        // 1. Sauvegarde persistante via le manager de JSON-DB
        let mut doc = serde_json::to_value(&rule)?;

        if let Some(obj) = doc.as_object_mut() {
            obj.insert("_target_collection".to_string(), json!(collection));
            obj.insert("id".to_string(), json!(rule.id));
        }

        self.db_manager.insert_raw("_system_rules", &doc).await?;

        // 2. Mise à jour du cache mémoire
        self.index_rule_in_cache(rule);
        Ok(())
    }

    fn index_rule_in_cache(&mut self, rule: Rule) {
        let deps = Analyzer::get_dependencies(&rule.expr);
        for dep in deps {
            self.dependency_cache
                .entry(dep)
                .or_default()
                .push(rule.id.clone());
        }
        self.rules_cache.insert(rule.id.clone(), rule);
    }

    /// Récupère les règles impactées par des changements (Mode Réactif)
    pub fn get_impacted_rules(
        &self,
        _collection: &str,
        changed_fields: &HashSet<String>,
    ) -> Vec<Rule> {
        let mut impacted_ids = HashSet::new();

        for field in changed_fields {
            if let Some(ids) = self.dependency_cache.get(field) {
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

    /// [NOUVEAU] Récupère toutes les règles ciblant une entité spécifique (Mode Validatif)
    /// Utilisé par le Bloc Cognitif pour valider un objet complet.
    pub fn get_rules_for_target(&self, target: &str) -> Vec<Rule> {
        self.rules_cache
            .values()
            .filter(|r| r.target == target)
            .cloned()
            .collect()
    }

    /// [NOUVEAU] Récupère l'ensemble des règles connues (Dump)
    pub fn get_all_rules(&self) -> Vec<Rule> {
        self.rules_cache.values().cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json_db::storage::{JsonDbConfig, StorageEngine};
    use crate::rules_engine::ast::Expr;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_store_idempotency_and_retrieval() {
        let dir = tempdir().unwrap();
        let config = JsonDbConfig::new(dir.path().to_path_buf());
        let storage = StorageEngine::new(config);
        let manager = CollectionsManager::new(&storage, "test_space", "test_db");
        manager.init_db().await.unwrap();

        let mut store = RuleStore::new(&manager);

        let rule1 = Rule {
            id: "r1".into(),
            target: "user_age".into(), // Cible spécifique
            expr: Expr::Val(json!(1)),
            description: None,
            severity: None,
        };

        let rule2 = Rule {
            id: "r2".into(),
            target: "system_status".into(),
            expr: Expr::Val(json!(2)),
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
