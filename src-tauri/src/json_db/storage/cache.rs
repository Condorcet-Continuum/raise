// FICHIER : src-tauri/src/json_db/storage/cache.rs

//! Module de gestion de cache LRU (Least Recently Used) thread-safe et asynchrone.

use crate::utils::prelude::*;

#[derive(Debug, Clone)]
struct CacheEntry<V> {
    value: V,
    expires_at: Option<TimeInstant>,
}

#[derive(Debug, Clone)]
pub struct Cache<K: Hashable + Eq, V> {
    // Le Mutex protège la structure LRU interne qui nécessite une mutation à chaque lecture
    store: SharedRef<AsyncMutex<MemoryCache<K, CacheEntry<V>>>>,
    default_ttl: Option<TimeDuration>,
}

impl<K, V> Cache<K, V>
where
    K: Hashable + Eq + Clone,
    V: Clone,
{
    pub fn new(capacity: usize, default_ttl: Option<TimeDuration>) -> Self {
        let cap = SafeSize::new(capacity).unwrap_or(SafeSize::new(100).unwrap());
        Self {
            store: SharedRef::new(AsyncMutex::new(MemoryCache::new(cap))),
            default_ttl,
        }
    }

    // ✅ NOUVEAU : Les méthodes passent en `async`
    pub async fn get(&self, key: &K) -> Option<V> {
        // Point de suspension asynchrone, ne bloque pas le thread !
        let mut guard = self.store.lock().await;

        if let Some(entry) = guard.get(key) {
            if let Some(expires_at) = entry.expires_at {
                if TimeInstant::now() > expires_at {
                    guard.pop(key);
                    return None;
                }
            }
            return Some(entry.value.clone());
        }
        None
    }

    pub async fn put(&self, key: K, value: V) {
        let now = TimeInstant::now();
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

    #[async_test] // ✅ Test asynchrone
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

    #[async_test]
    async fn test_cache_expiration() {
        let cache = Cache::new(10, Some(TimeDuration::from_millis(50)));
        cache.put("k1", 100).await;
        assert_eq!(cache.get(&"k1").await, Some(100));

        sleep_async(TimeDuration::from_millis(60)).await; // Pause asynchrone
        assert_eq!(cache.get(&"k1").await, None);
    }
}
