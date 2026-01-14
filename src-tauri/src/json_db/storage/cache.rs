// FICHIER : src-tauri/src/json_db/storage/cache.rs

//! Module de gestion de cache générique en mémoire.

use std::collections::HashMap;
use std::hash::Hash;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
struct CacheEntry<V> {
    value: V,
    #[allow(dead_code)]
    created_at: Instant,
    #[allow(dead_code)]
    last_accessed: Instant,
    expires_at: Option<Instant>,
}

#[derive(Debug, Clone)]
pub struct Cache<K, V> {
    // Arc<RwLock> permet le clonage léger et l'accès concurrent
    store: Arc<RwLock<HashMap<K, CacheEntry<V>>>>,
    capacity: usize,
    default_ttl: Option<Duration>,
}

impl<K, V> Cache<K, V>
where
    K: Eq + Hash + Clone,
    V: Clone,
{
    pub fn new(capacity: usize, default_ttl: Option<Duration>) -> Self {
        Self {
            store: Arc::new(RwLock::new(HashMap::new())),
            capacity,
            default_ttl,
        }
    }

    pub fn get(&self, key: &K) -> Option<V> {
        let store = self.store.read().ok()?;
        if let Some(entry) = store.get(key) {
            if let Some(expires_at) = entry.expires_at {
                if Instant::now() > expires_at {
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

        let entry = CacheEntry {
            value,
            created_at: now,
            last_accessed: now,
            expires_at,
        };

        if let Ok(mut guard) = self.store.write() {
            // Nettoyage paresseux si capacité atteinte
            if guard.len() >= self.capacity && !guard.contains_key(&key) {
                // 1. Supprimer les expirés
                guard.retain(|_, v| v.expires_at.map(|exp| exp > now).unwrap_or(true));

                // 2. Si toujours plein, éviction arbitraire (pour simplifier, on prend le premier)
                // Note: Une vraie LRU nécessiterait une LinkedHashMap ou structure additionnelle
                if guard.len() >= self.capacity {
                    if let Some(k) = guard.keys().next().cloned() {
                        guard.remove(&k);
                    }
                }
            }
            guard.insert(key, entry);
        }
    }

    pub fn remove(&self, key: &K) {
        if let Ok(mut guard) = self.store.write() {
            guard.remove(key);
        }
    }

    pub fn clear(&self) {
        if let Ok(mut guard) = self.store.write() {
            guard.clear();
        }
    }

    pub fn len(&self) -> usize {
        self.store.read().map(|g| g.len()).unwrap_or(0)
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
    fn test_cache_basic_ops() {
        let cache = Cache::new(10, None);
        cache.put("k1", 100);
        assert_eq!(cache.get(&"k1"), Some(100));

        cache.remove(&"k1");
        assert_eq!(cache.get(&"k1"), None);
    }

    #[test]
    fn test_cache_expiration() {
        let cache = Cache::new(10, Some(Duration::from_millis(50)));
        cache.put("k1", 100);
        assert_eq!(cache.get(&"k1"), Some(100));

        thread::sleep(Duration::from_millis(60));
        assert_eq!(cache.get(&"k1"), None);
    }

    #[test]
    fn test_cache_eviction() {
        // Capacité de 2
        let cache = Cache::new(2, None);
        cache.put("k1", 1);
        cache.put("k2", 2);

        // Ajout d'un 3ème, doit éjecter k1 ou k2
        cache.put("k3", 3);

        assert_eq!(cache.len(), 2);
        // Au moins un des anciens a disparu
        let has_k1 = cache.get(&"k1").is_some();
        let has_k2 = cache.get(&"k2").is_some();
        assert!(!(has_k1 && has_k2));
    }
}
