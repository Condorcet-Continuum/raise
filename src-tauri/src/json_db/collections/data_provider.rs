// FICHIER : src-tauri/src/json_db/collections/data_provider.rs

use crate::json_db::collections::collection;
use crate::json_db::storage::JsonDbConfig;
use crate::rules_engine::DataProvider;

// FAÇADE UNIQUE
use crate::utils::{async_trait, json::Value, AsyncRwLock, HashMap};

/// Un DataProvider qui met en cache les documents lus pour la durée de son existence.
/// Indispensable pour garantir la cohérence des lookups lors de l'exécution des règles métier.
pub struct CachedDataProvider<'a> {
    config: &'a JsonDbConfig,
    space: &'a str,
    db: &'a str,
    /// Cache interne : (Collection, ID) -> Document.
    /// RwLock permet des lectures asynchrones concurrentes.
    doc_cache: AsyncRwLock<HashMap<(String, String), Option<Value>>>,
}

impl<'a> CachedDataProvider<'a> {
    pub fn new(config: &'a JsonDbConfig, space: &'a str, db: &'a str) -> Self {
        Self {
            config,
            space,
            db,
            doc_cache: AsyncRwLock::new(HashMap::new()),
        }
    }

    /// Charge un document depuis le cache ou effectue une lecture disque asynchrone.
    async fn get_document(&self, collection: &str, id: &str) -> Option<Value> {
        let key = (collection.to_string(), id.to_string());

        // 1. Tentative de lecture rapide depuis le cache
        {
            let cache = self.doc_cache.read().await;
            if let Some(cached_doc) = cache.get(&key) {
                return cached_doc.clone();
            }
        }

        // 2. Lecture I/O asynchrone (point d'arrêt .await)
        let doc = collection::read_document(self.config, self.space, self.db, collection, id)
            .await
            .ok();

        // 3. Mise à jour du cache pour les prochains accès
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
    use crate::utils::{
        io::{self, tempdir},
        json::{self, json},
    };

    #[tokio::test]
    async fn test_cached_provider_memoization() {
        let dir = tempdir().unwrap();
        let config = JsonDbConfig::new(dir.path().to_path_buf());
        let space = "test_space";
        let db = "test_db";

        let col_path = config.db_collection_path(space, db, "users");
        io::create_dir_all(&col_path).await.unwrap();

        // Préparation du document disque
        let id = "u1";
        let initial_json = json!({ "id": id, "score": 100 });
        io::write(
            col_path.join(format!("{}.json", id)),
            json::stringify(&initial_json).expect("Erreur de sérialisation"),
        )
        .await
        .expect("Erreur d'écriture");

        let provider = CachedDataProvider::new(&config, space, db);

        // Première lecture : doit charger depuis le disque
        let val = provider.get_value("users", id, "score").await;
        assert_eq!(val, Some(json!(100)));

        // Altération physique du fichier
        io::write(
            col_path.join(format!("{}.json", id)),
            json::stringify(&json!({ "id": id, "score": 999 })).expect("Erreur de sérialisation"),
        )
        .await
        .expect("Erreur d'écriture");

        // Deuxième lecture : doit renvoyer la valeur du cache (100) et non 999
        let cached_val = provider.get_value("users", id, "score").await;
        assert_eq!(cached_val, Some(json!(100)));
    }

    #[tokio::test]
    async fn test_cached_provider_missing_doc() {
        let dir = tempdir().unwrap();
        let config = JsonDbConfig::new(dir.path().to_path_buf());
        let provider = CachedDataProvider::new(&config, "s", "d");

        // Test d'un document inexistant
        let val = provider.get_value("ghost", "none", "any").await;
        assert!(val.is_none());
    }
}
