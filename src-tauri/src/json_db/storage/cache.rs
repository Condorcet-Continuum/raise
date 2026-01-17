// FICHIER : src-tauri/src/json_db/storage/cache.rs

//! Module de gestion de cache LRU (Least Recently Used) thread-safe.

use lru::LruCache;
use std::hash::Hash;
use std::num::NonZeroUsize;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
struct CacheEntry<V> {
    value: V,
    expires_at: Option<Instant>,
}

#[derive(Debug, Clone)]
pub struct Cache<K: Hash + Eq, V> {
    // Utilisation de Mutex car LruCache nécessite une mutation à chaque accès (get)
    // pour réordonner les éléments (promotion de l'élément accédé).
    store: Arc<Mutex<LruCache<K, CacheEntry<V>>>>,
    default_ttl: Option<Duration>,
}

impl<K, V> Cache<K, V>
where
    K: Hash + Eq + Clone,
    V: Clone,
{
    pub fn new(capacity: usize, default_ttl: Option<Duration>) -> Self {
        // LruCache utilise NonZeroUsize pour garantir une capacité valide
        let cap = NonZeroUsize::new(capacity).unwrap_or(NonZeroUsize::new(100).unwrap());
        Self {
            store: Arc::new(Mutex::new(LruCache::new(cap))),
            default_ttl,
        }
    }

    pub fn get(&self, key: &K) -> Option<V> {
        let mut guard = self.store.lock().ok()?;

        // LruCache::get promeut l'élément en haut de la pile (MRU)
        if let Some(entry) = guard.get(key) {
            // Vérification de l'expiration
            if let Some(expires_at) = entry.expires_at {
                if Instant::now() > expires_at {
                    // Suppression manuelle si expiré
                    guard.pop(key);
                    return None;
                }
            }
            return Some(entry.value.clone());
        }
        None
    }

    pub fn put(&self, key: K, value: V) {
        let now = Instant::now();
        let expires_at = self.default_ttl.map(|ttl| now + ttl);

        let entry = CacheEntry { value, expires_at };

        if let Ok(mut guard) = self.store.lock() {
            // L'insertion gère automatiquement l'éviction si la capacité est dépassée
            guard.put(key, entry);
        }
    }

    pub fn remove(&self, key: &K) {
        if let Ok(mut guard) = self.store.lock() {
            guard.pop(key);
        }
    }

    pub fn clear(&self) {
        if let Ok(mut guard) = self.store.lock() {
            guard.clear();
        }
    }

    pub fn len(&self) -> usize {
        self.store.lock().map(|g| g.len()).unwrap_or(0)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn test_cache_lru_behavior() {
        // Capacité de 2
        let cache = Cache::new(2, None);
        cache.put("k1", 100);
        cache.put("k2", 200);

        // Accéder à k1 pour le rendre "récent"
        cache.get(&"k1");

        // Ajouter k3, devrait éjecter k2 (le moins récemment utilisé)
        cache.put("k3", 300);

        assert_eq!(cache.get(&"k1"), Some(100));
        assert_eq!(cache.get(&"k2"), None);
        assert_eq!(cache.get(&"k3"), Some(300));
    }

    #[test]
    fn test_cache_expiration() {
        let cache = Cache::new(10, Some(Duration::from_millis(50)));
        cache.put("k1", 100);
        assert_eq!(cache.get(&"k1"), Some(100));

        thread::sleep(Duration::from_millis(60));
        assert_eq!(cache.get(&"k1"), None);
    }
}
