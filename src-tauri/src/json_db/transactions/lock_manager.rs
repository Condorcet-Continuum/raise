use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Gestionnaire de verrous simple (granularité : Collection)
#[derive(Debug, Default, Clone)]
pub struct LockManager {
    // Clé = "space/db/collection"
    locks: Arc<RwLock<HashMap<String, Arc<RwLock<()>>>>>,
}

impl LockManager {
    pub fn new() -> Self {
        Self {
            locks: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Récupère un verrou d'écriture pour une collection
    pub fn get_write_lock(&self, space: &str, db: &str, collection: &str) -> Arc<RwLock<()>> {
        let key = format!("{}/{}/{}", space, db, collection);
        let mut map = self.locks.write().unwrap();
        map.entry(key)
            .or_insert_with(|| Arc::new(RwLock::new(())))
            .clone()
    }
}
