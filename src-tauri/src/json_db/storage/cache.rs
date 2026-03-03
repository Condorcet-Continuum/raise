// FICHIER : src-tauri/src/json_db/storage/cache.rs

//! Module de gestion de cache LRU (Least Recently Used) thread-safe et asynchrone.

use crate::utils::{Arc, AsyncMutex, Duration, Hash, Instant, LruCache, NonZeroUsize};

#[derive(Debug, Clone)]
struct CacheEntry<V> {
    value: V,
    expires_at: Option<Instant>,
}

#[derive(Debug, Clone)]
pub struct Cache<K: Hash + Eq, V> {
    // Le Mutex protège la structure LRU interne qui nécessite une mutation à chaque lecture
    store: Arc<AsyncMutex<LruCache<K, CacheEntry<V>>>>,
    default_ttl: Option<Duration>,
}

impl<K, V> Cache<K, V>
where
    K: Hash + Eq + Clone,
    V: Clone,
{
    pub fn new(capacity: usize, default_ttl: Option<Duration>) -> Self {
        let cap = NonZeroUsize::new(capacity).unwrap_or(NonZeroUsize::new(100).unwrap());
        Self {
            store: Arc::new(AsyncMutex::new(LruCache::new(cap))),
            default_ttl,
        }
    }

    // ✅ NOUVEAU : Les méthodes passent en `async`
    pub async fn get(&self, key: &K) -> Option<V> {
        // Point de suspension asynchrone, ne bloque pas le thread !
        let mut guard = self.store.lock().await;

        if let Some(entry) = guard.get(key) {
            if let Some(expires_at) = entry.expires_at {
                if Instant::now() > expires_at {
                    guard.pop(key);
                    return None;
                }
            }
            return Some(entry.value.clone());
        }
        None
    }

    pub async fn put(&self, key: K, value: V) {
        let now = Instant::now();
        let expires_at = self.default_ttl.map(|ttl| now + ttl);

        let entry = CacheEntry { value, expires_at };

        let mut guard = self.store.lock().await;
        guard.put(key, entry);
    }

    pub async fn remove(&self, key: &K) {
        let mut guard = self.store.lock().await;
        guard.pop(key);
    }

    pub async fn clear(&self) {
        let mut guard = self.store.lock().await;
        guard.clear();
    }

    pub async fn len(&self) -> usize {
        let guard = self.store.lock().await;
        guard.len()
    }

    pub async fn is_empty(&self) -> bool {
        self.len().await == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::sleep; // Remplacement du sleep synchrone

    #[tokio::test] // ✅ Test asynchrone
    async fn test_cache_lru_behavior() {
        let cache = Cache::new(2, None);
        cache.put("k1", 100).await;
        cache.put("k2", 200).await;

        // Accéder à k1 pour le rendre "récent"
        cache.get(&"k1").await;

        // Ajouter k3, devrait éjecter k2
        cache.put("k3", 300).await;

        assert_eq!(cache.get(&"k1").await, Some(100));
        assert_eq!(cache.get(&"k2").await, None);
        assert_eq!(cache.get(&"k3").await, Some(300));
    }

    #[tokio::test]
    async fn test_cache_expiration() {
        let cache = Cache::new(10, Some(Duration::from_millis(50)));
        cache.put("k1", 100).await;
        assert_eq!(cache.get(&"k1").await, Some(100));

        sleep(Duration::from_millis(60)).await; // Pause asynchrone
        assert_eq!(cache.get(&"k1").await, None);
    }
}
