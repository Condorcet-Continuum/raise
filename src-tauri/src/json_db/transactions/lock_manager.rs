// FICHIER : src-tauri/src/json_db/transactions/lock_manager.rs

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

// ============================================================================
// TESTS UNITAIRES
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc; // Ajout pour la synchronisation
    use std::thread;
    use std::time::Duration;

    #[test]
    fn test_lock_concurrency() {
        let manager = LockManager::new();
        let lock1 = manager.get_write_lock("s", "d", "users");
        let lock2 = manager.get_write_lock("s", "d", "users");

        // Canal pour signaler que le thread 1 a bien acquis le verrou
        let (tx, rx) = mpsc::channel();

        // Simulation : Thread 1 prend le verrou
        let handle = thread::spawn(move || {
            let _guard = lock1.write().unwrap();

            // On signale au main thread qu'on a le verrou
            tx.send(()).unwrap();

            // On garde le verrou 50ms
            thread::sleep(Duration::from_millis(50));
        });

        // Le main thread attend obligatoirement que le thread 1 ait le verrou
        rx.recv().unwrap();

        // Thread 2 essaie de prendre le verrou (doit attendre)
        let start = std::time::Instant::now();
        let _guard = lock2.write().unwrap();
        let duration = start.elapsed();

        handle.join().unwrap();

        // Le verrou fonctionne si on a dû attendre
        assert!(duration >= Duration::from_millis(50));
    }
}
