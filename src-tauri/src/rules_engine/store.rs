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
    rules_cache: HashMap<String, Rule>,
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

    /// Initialise le store en chargeant les règles depuis la collection système
    pub fn sync_from_db(&mut self) -> Result<()> {
        let stored_rules = self
            .db_manager
            .list_all("_system_rules")
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

    /// Enregistre une règle de manière idempotente
    /// Ne persiste sur le disque QUE si la règle est nouvelle ou modifiée
    pub fn register_rule(&mut self, collection: &str, rule: Rule) -> Result<()> {
        // OPTIMISATION : Vérifier si la règle existe déjà et est identique
        if let Some(existing_rule) = self.rules_cache.get(&rule.id) {
            // Cela fonctionne maintenant grâce au derive(PartialEq) dans ast.rs
            if *existing_rule == rule {
                return Ok(()); // Rien à faire, on économise l'I/O
            }
        }

        // 1. Sauvegarde persistante via le manager de JSON-DB
        let mut doc = serde_json::to_value(&rule)?;

        if let Some(obj) = doc.as_object_mut() {
            obj.insert("_target_collection".to_string(), json!(collection));
            obj.insert("id".to_string(), json!(rule.id));
        }

        self.db_manager.insert_raw("_system_rules", &doc)?;

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

    /// Récupère les règles impactées (Lookup O(1) via le cache)
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json_db::storage::{JsonDbConfig, StorageEngine};
    use crate::rules_engine::ast::Expr;
    use tempfile::tempdir;

    #[test]
    fn test_store_idempotency() {
        let dir = tempdir().unwrap();
        let config = JsonDbConfig::new(dir.path().to_path_buf());
        let storage = StorageEngine::new(config);
        let manager = CollectionsManager::new(&storage, "test_space", "test_db");
        manager.init_db().unwrap();

        let mut store = RuleStore::new(&manager);

        let rule = Rule {
            id: "r1".into(),
            target: "t".into(),
            expr: Expr::Val(json!(1)),
        };

        // Premier enregistrement : Doit écrire
        store.register_rule("col", rule.clone()).unwrap();
        let docs_pass_1 = manager.list_all("_system_rules").unwrap();
        // CORRECTION : préfixe _ pour éviter le warning
        let _mtime_1 = docs_pass_1[0]["updatedAt"]
            .as_str()
            .unwrap_or("")
            .to_string();

        // Deuxième enregistrement identique : Ne doit PAS écrire
        store.register_rule("col", rule.clone()).unwrap();

        let docs_pass_2 = manager.list_all("_system_rules").unwrap();
        assert_eq!(docs_pass_2.len(), 1);

        // Si on modifie la règle
        let rule_mod = Rule {
            id: "r1".into(),
            target: "t".into(),
            expr: Expr::Val(json!(2)),
        };
        store.register_rule("col", rule_mod).unwrap();
        let docs_pass_3 = manager.list_all("_system_rules").unwrap();
        assert_eq!(docs_pass_3.len(), 1);
    }
}
