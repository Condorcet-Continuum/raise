// FICHIER : src-tauri/src/json_db/collections/data_provider.rs

use crate::json_db::collections::collection;
use crate::json_db::storage::JsonDbConfig;
use crate::rules_engine::DataProvider;
use serde_json::Value;
use std::cell::RefCell;
use std::collections::HashMap;

/// Un DataProvider qui met en cache les documents lus pour la durée de son existence.
/// Idéal pour une transaction où plusieurs règles peuvent accéder aux mêmes données référentielles.
pub struct CachedDataProvider<'a> {
    config: &'a JsonDbConfig,
    space: &'a str,
    db: &'a str,
    /// Cache : (Collection, ID) -> Document complet
    /// On utilise RefCell pour permettre la modification du cache via une référence immuable (&self)
    doc_cache: RefCell<HashMap<(String, String), Option<Value>>>,
}

impl<'a> CachedDataProvider<'a> {
    pub fn new(config: &'a JsonDbConfig, space: &'a str, db: &'a str) -> Self {
        Self {
            config,
            space,
            db,
            doc_cache: RefCell::new(HashMap::new()),
        }
    }

    /// Charge un document depuis le cache ou le disque
    fn get_document(&self, collection: &str, id: &str) -> Option<Value> {
        let key = (collection.to_string(), id.to_string());

        // 1. Vérification rapide dans le cache
        if let Some(cached_doc) = self.doc_cache.borrow().get(&key) {
            return cached_doc.clone();
        }

        // 2. Lecture disque (coûteuse)
        let doc = collection::read_document(self.config, self.space, self.db, collection, id).ok();

        // 3. Mise en cache
        self.doc_cache.borrow_mut().insert(key, doc.clone());
        doc
    }
}

impl<'a> DataProvider for CachedDataProvider<'a> {
    fn get_value(&self, collection: &str, id: &str, field: &str) -> Option<Value> {
        // On récupère le document entier (qui est peut-être déjà en cache)
        if let Some(doc) = self.get_document(collection, id) {
            // On applique le pointeur JSON pour récupérer le champ spécifique
            let ptr = if field.starts_with('/') {
                field.to_string()
            } else {
                format!("/{}", field.replace('.', "/"))
            };
            return doc.pointer(&ptr).cloned();
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json_db::storage::StorageEngine;
    use serde_json::json;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_cached_provider_memoization() {
        let dir = tempdir().unwrap();
        let config = JsonDbConfig::new(dir.path().to_path_buf());
        // Init dummy storage pour créer l'arborescence si besoin
        let _ = StorageEngine::new(config.clone());

        let space = "test_space";
        let db = "test_db";
        let col_path = config.db_collection_path(space, db, "users");
        fs::create_dir_all(&col_path).unwrap();

        // Écriture initiale sur le disque
        let user_json = json!({ "id": "u1", "info": { "age": 30 } });
        fs::write(
            col_path.join("u1.json"),
            serde_json::to_string(&user_json).unwrap(),
        )
        .unwrap();

        let provider = CachedDataProvider::new(&config, space, db);

        // Lecture 1 (Disque -> Cache)
        let val1 = provider.get_value("users", "u1", "info.age");
        assert_eq!(val1, Some(json!(30)));

        // Modification disque "en douce" (simulation d'une modif externe concurrente)
        let hacked_json = json!({ "id": "u1", "info": { "age": 999 } });
        fs::write(
            col_path.join("u1.json"),
            serde_json::to_string(&hacked_json).unwrap(),
        )
        .unwrap();

        // Lecture 2 (Doit venir du Cache interne du provider, donc 30 et pas 999)
        // Cela garantit la cohérence des données au sein d'une même transaction de règles.
        let val2 = provider.get_value("users", "u1", "info.age");
        assert_eq!(val2, Some(json!(30)));
    }
}
