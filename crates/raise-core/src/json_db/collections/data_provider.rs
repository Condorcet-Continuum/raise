// FICHIER : src-tauri/src/json_db/collections/data_provider.rs

use crate::json_db::collections::collection;
use crate::json_db::storage::StorageEngine;
use crate::rules_engine::DataProvider;

// FAÇADE UNIQUE
use crate::utils::prelude::*;

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
    doc_cache: AsyncRwLock<UnorderedMap<(String, String), Option<JsonValue>>>,
}

impl<'a> CachedDataProvider<'a> {
    pub fn new(storage: &'a StorageEngine, space: &'a str, db: &'a str) -> Self {
        Self {
            storage,
            space,
            db,
            doc_cache: AsyncRwLock::new(UnorderedMap::new()),
        }
    }

    /// Charge un document depuis le cache L1, ou délègue au StorageEngine (Cache L2 / Disque).
    async fn get_document(&self, collection: &str, id: &str) -> Option<JsonValue> {
        let key = (collection.to_string(), id.to_string());

        // 1. Tentative de lecture ultra-rapide et isolée depuis le cache L1
        {
            let cache = self.doc_cache.read().await;
            if let Some(cached_doc) = cache.get(&key) {
                return cached_doc.clone();
            }
        }

        // 2. Cache Miss L1 -> On interroge le StorageEngine (qui a son propre Cache LRU)
        let doc = match collection::read_document(self.storage, self.space, self.db, collection, id)
            .await
        {
            Ok(json) => Some(json),
            Err(e) => {
                // En cas d'erreur physique, on logue mais on renvoie None pour ne pas bloquer les règles
                user_warn!(
                    "WRN_DATA_PROVIDER_L1_FAIL",
                    json_value!({ "error": e.to_string(), "id": id, "coll": collection })
                );
                None
            }
        };

        // 3. Mise à jour du cache L1 pour garantir la cohérence des prochains accès
        let mut cache = self.doc_cache.write().await;
        cache.insert(key, doc.clone());
        doc
    }
}

#[async_interface]
impl<'a> DataProvider for CachedDataProvider<'a> {
    /// Récupère une valeur spécifique via un chemin JSON (ex: "profile.email").
    async fn get_value(&self, collection: &str, id: &str, field: &str) -> Option<JsonValue> {
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

    #[async_test]
    async fn test_cached_provider_memoization() -> RaiseResult<()> {
        let dir = match tempdir() {
            Ok(d) => d,
            Err(e) => panic!("TempDir fail: {:?}", e),
        };
        let config = JsonDbConfig::new(dir.path().to_path_buf());

        // 🎯 FIX E0308 : StorageEngine::new renvoie RaiseResult
        let storage = match StorageEngine::new(config) {
            Ok(s) => s,
            Err(e) => return Err(e),
        };

        let (space, db, coll, id) = ("test_space", "test_db", "users", "u1");

        // Préparation propre (Propagation via ?)
        collection::create_collection_if_missing(&storage.config, space, db, coll).await?;
        storage
            .write_document(
                space,
                db,
                coll,
                id,
                &json_value!({ "id": id, "score": 100 }),
            )
            .await?;

        let provider = CachedDataProvider::new(&storage, space, db);

        // Lecture 1 (L1 Empty -> L2 Hit)
        let val = provider.get_value(coll, id, "score").await;
        assert_eq!(val, Some(json_value!(100)));

        // Altération directe en L2
        storage
            .write_document(
                space,
                db,
                coll,
                id,
                &json_value!({ "id": id, "score": 999 }),
            )
            .await?;

        // Lecture 2 (L1 Hit) : Doit rester à 100 pour la cohérence de la requête
        let cached_val = provider.get_value(coll, id, "score").await;
        assert_eq!(cached_val, Some(json_value!(100)));

        Ok(())
    }

    #[async_test]
    async fn test_cached_provider_missing_doc() -> RaiseResult<()> {
        // 1. Gestion propre du dossier temporaire
        let dir = match tempdir() {
            Ok(d) => d,
            Err(e) => panic!("Échec TempDir : {:?}", e),
        };
        let config = JsonDbConfig::new(dir.path().to_path_buf());

        // 2. 🎯 FIX E0308 : Extraction de l'instance du StorageEngine
        // StorageEngine::new(config) renvoie maintenant un RaiseResult.
        let storage = match StorageEngine::new(config) {
            Ok(s) => s,
            Err(e) => return Err(e),
        };

        // 3. Instanciation du Provider avec la référence correcte (&StorageEngine)
        let provider = CachedDataProvider::new(&storage, "s", "d");

        // 4. Test d'un document inexistant
        // get_value renvoie Option<JsonValue>, on vérifie l'absence.
        let val = provider.get_value("ghost", "none", "any").await;
        assert!(val.is_none());

        Ok(())
    }
}
