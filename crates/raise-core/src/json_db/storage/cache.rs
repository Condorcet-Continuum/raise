// FICHIER : src-tauri/src/json_db/storage/cache.rs

//! Module de gestion de cache LRU (Least Recently Used) thread-safe ultra-performant.

use crate::utils::prelude::*;

#[derive(Debug, Clone)]
struct CacheEntry<V> {
    value: V,
    expires_at: Option<TimeInstant>,
}

#[derive(Debug, Clone)]
pub struct Cache<K: Hashable + Eq, V> {
    // 🎯 FIX : On utilise un SyncMutex car les opérations RAM
    // prennent des nanosecondes et ne doivent pas suspendre le runtime asynchrone !
    store: SharedRef<SyncMutex<MemoryCache<K, CacheEntry<V>>>>,
    default_ttl: Option<TimeDuration>,
}

impl<K, V> Cache<K, V>
where
    K: Hashable + Eq + Clone,
    V: Clone,
{
    pub fn new(capacity: usize, default_ttl: Option<TimeDuration>) -> RaiseResult<Self> {
        // 1. Validation de la capacité (SafeSize::new renvoie Option<NonZeroUsize>)
        let cap = match SafeSize::new(capacity) {
            Some(s) => s,
            None => {
                raise_error!(
                    "ERR_STORAGE_CACHE_INIT_FAILED",
                    context = json_value!({ "requested_capacity": capacity })
                )
            }
        };

        // 2. Création de l'instance (MemoryCache::new attend SafeSize/NonZeroUsize)
        Ok(Self {
            store: SharedRef::new(SyncMutex::new(MemoryCache::new(cap))),
            default_ttl,
        })
    }

    /// Acquisition sécurisée du verrou synchrone (Nanosecondes).
    fn acquire_lock(&self) -> RaiseResult<SyncMutexGuard<'_, MemoryCache<K, CacheEntry<V>>>> {
        match self.store.lock() {
            Ok(guard) => Ok(guard),
            Err(e) => {
                raise_error!("ERR_STORAGE_CACHE_POISONED", error = e.to_string())
            }
        }
    }

    pub fn get(&self, key: &K) -> RaiseResult<Option<V>> {
        let mut guard = self.acquire_lock()?;

        let entry = match guard.get(key) {
            Some(e) => e,
            None => return Ok(None),
        };

        if let Some(expires_at) = entry.expires_at {
            if TimeInstant::now() > expires_at {
                guard.pop(key);
                return Ok(None);
            }
        }

        Ok(Some(entry.value.clone()))
    }

    pub fn put(&self, key: K, value: V) -> RaiseResult<()> {
        let mut guard = self.acquire_lock()?;

        let expires_at = self.default_ttl.map(|ttl| TimeInstant::now() + ttl);

        let entry = CacheEntry { value, expires_at };
        guard.put(key, entry);

        Ok(())
    }

    pub fn remove(&self, key: &K) -> RaiseResult<()> {
        let mut guard = self.acquire_lock()?;

        guard.pop(key);
        Ok(())
    }

    pub fn clear(&self) -> RaiseResult<()> {
        let mut guard = self.acquire_lock()?;

        guard.clear();
        Ok(())
    }

    pub fn len(&self) -> RaiseResult<usize> {
        let guard = self.acquire_lock()?;

        Ok(guard.len())
    }

    pub fn is_empty(&self) -> RaiseResult<bool> {
        match self.len() {
            Ok(l) => Ok(l == 0),
            Err(e) => Err(e),
        }
    }
}

// Les tests associés doivent également perdre leurs `.await` lors des appels au cache.
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_lru_behavior() -> RaiseResult<()> {
        let cache = match Cache::new(2, None) {
            Ok(c) => c,
            Err(e) => return Err(e),
        };

        cache.put("k1", 100)?;
        cache.put("k2", 200)?;

        let _ = cache.get(&"k1")?;
        cache.put("k3", 300)?;

        assert_eq!(cache.get(&"k1")?, Some(100));
        assert_eq!(cache.get(&"k2")?, None);
        assert_eq!(cache.get(&"k3")?, Some(300));

        Ok(())
    }
}
