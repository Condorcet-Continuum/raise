// FICHIER : src-tauri/src/json_db/collections/data_provider.rs

use crate::json_db::collections::collection;
use crate::json_db::storage::StorageEngine;
use crate::rules_engine::DataProvider;

// FAÇADE UNIQUE
use crate::utils::{async_trait, json::Value, AsyncRwLock, HashMap};

/// Un DataProvider qui met en cache les documents lus pour la durée de son existence.
/// Agit comme un "Cache de Niveau 1" (L1) isolé par requête pour garantir la cohérence
/// des lookups lors de l'exécution des règles métier, tout en s'appuyant sur le
/// StorageEngine (Cache L2 Global) pour les accès disques.
pub struct CachedDataProvider<'a> {
    storage: &'a StorageEngine,
    space: &'a str,
    db: &'a str,
    /// Cache interne L1 : (Collection, ID) -> Document.
    /// RwLock permet des lectures asynchrones concurrentes au sein d'une même évaluation.
    doc_cache: AsyncRwLock<HashMap<(String, String), Option<Value>>>,
}

impl<'a> CachedDataProvider<'a> {
    pub fn new(storage: &'a StorageEngine, space: &'a str, db: &'a str) -> Self {
        Self {
            storage,
            space,
            db,
            doc_cache: AsyncRwLock::new(HashMap::new()),
        }
    }

    /// Charge un document depuis le cache L1, ou délègue au StorageEngine (Cache L2 / Disque).
    async fn get_document(&self, collection: &str, id: &str) -> Option<Value> {
        let key = (collection.to_string(), id.to_string());

        // 1. Tentative de lecture ultra-rapide et isolée depuis le cache L1
        {
            let cache = self.doc_cache.read().await;
            if let Some(cached_doc) = cache.get(&key) {
                return cached_doc.clone();
            }
        }

        // 2. Cache Miss L1 -> On interroge le StorageEngine (qui a son propre Cache LRU)
        let doc = collection::read_document(self.storage, self.space, self.db, collection, id)
            .await
            .ok();

        // 3. Mise à jour du cache L1 pour garantir la cohérence des prochains accès
        let mut cache = self.doc_cache.write().await;
        cache.insert(key, doc.clone());
        doc
    }
}

#[async_trait]
impl<'a> DataProvider for CachedDataProvider<'a> {
    /// Récupère une valeur spécifique via un chemin JSON (ex: "profile.email").
    async fn get_value(&self, collection: &str, id: &str, field: &str) -> Option<Value> {
        if let Some(doc) = self.get_document(collection, id).await {
            // Conversion du chemin pointé (a.b) en pointeur JSON (/a/b)
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
    use crate::json_db::storage::JsonDbConfig;
    use crate::utils::{io::tempdir, json::json};

    #[tokio::test]
    async fn test_cached_provider_memoization() {
        let dir = tempdir().unwrap();
        let config = JsonDbConfig::new(dir.path().to_path_buf());

        // Initialisation du StorageEngine pour le test
        let storage = StorageEngine::new(config);
        let space = "test_space";
        let db = "test_db";

        // Création de la collection via l'API pour être propre
        collection::create_collection_if_missing(&storage.config, space, db, "users")
            .await
            .unwrap();

        // Préparation du document initial via le StorageEngine
        let id = "u1";
        let initial_json = json!({ "id": id, "score": 100 });
        storage
            .write_document(space, db, "users", id, &initial_json)
            .await
            .unwrap();

        let provider = CachedDataProvider::new(&storage, space, db);

        // Première lecture : doit charger depuis le StorageEngine (et peupler le L1)
        let val = provider.get_value("users", id, "score").await;
        assert_eq!(val, Some(json!(100)));

        // Altération du document via le StorageEngine (met à jour le L2 et le disque)
        let altered_json = json!({ "id": id, "score": 999 });
        storage
            .write_document(space, db, "users", id, &altered_json)
            .await
            .unwrap();

        // Deuxième lecture via le Provider : doit renvoyer la valeur du cache L1 (100)
        // et NON 999, prouvant ainsi la "snapshot isolation" durant la vie du Provider !
        let cached_val = provider.get_value("users", id, "score").await;
        assert_eq!(cached_val, Some(json!(100)));
    }

    #[tokio::test]
    async fn test_cached_provider_missing_doc() {
        let dir = tempdir().unwrap();
        let config = JsonDbConfig::new(dir.path().to_path_buf());
        let storage = StorageEngine::new(config);

        let provider = CachedDataProvider::new(&storage, "s", "d");

        // Test d'un document inexistant
        let val = provider.get_value("ghost", "none", "any").await;
        assert!(val.is_none());
    }
}
