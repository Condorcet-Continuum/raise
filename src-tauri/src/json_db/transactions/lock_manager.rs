// FICHIER : src-tauri/src/json_db/transactions/lock_manager.rs

use crate::utils::{
    Arc,         // std::sync::Arc
    AsyncRwLock, // tokio::sync::RwLock
    HashMap,     // std::collections::HashMap
    RwLock,      // std::sync::RwLock
};

/// Gestionnaire de verrous simple (granularité : Collection)
/// Utilise des verrous ASYNCHRONES (Tokio) pour être compatible avec .await
#[derive(Debug, Default, Clone)]
pub struct LockManager {
    // Clé = "space/db/collection"
    // Le RwLock EXTERNE (Std) protège la Map (accès rapide mémoire)
    // Le RwLock INTERNE (Tokio) protège la Collection (attente longue async)
    locks: Arc<RwLock<HashMap<String, Arc<AsyncRwLock<()>>>>>,
}

impl LockManager {
    pub fn new() -> Self {
        Self {
            locks: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Récupère un verrou d'écriture ASYNC pour une collection
    /// Retourne un Arc<tokio::sync::RwLock<()>>
    pub fn get_write_lock(&self, space: &str, db: &str, collection: &str) -> Arc<AsyncRwLock<()>> {
        let key = format!("{}/{}/{}", space, db, collection);

        // 1. On verrouille la map juste le temps de récupérer/créer l'entrée
        let mut map = self.locks.write().unwrap();

        map.entry(key)
            .or_insert_with(|| Arc::new(AsyncRwLock::new(())))
            .clone()
    }
}

// ============================================================================
// TESTS UNITAIRES (ASYNC)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::{mpsc, sleep, Duration};

    #[tokio::test]
    async fn test_lock_concurrency() {
        let manager = LockManager::new();
        let lock1 = manager.get_write_lock("s", "d", "users");
        let lock2 = manager.get_write_lock("s", "d", "users");

        // Canal pour signaler que la tâche 1 a bien acquis le verrou
        let (tx, mut rx) = mpsc::channel(1);

        // Simulation : Tâche 1 prend le verrou
        let handle = tokio::spawn(async move {
            // Ici on utilise .write().await
            let _guard = lock1.write().await;

            // On signale qu'on a le verrou
            tx.send(()).await.unwrap();

            // On garde le verrou 50ms
            sleep(Duration::from_millis(50)).await;
        });

        // Le main thread attend que la tâche 1 ait le verrou
        rx.recv().await.unwrap();

        // Tâche 2 essaie de prendre le verrou (doit attendre)
        let start = std::time::Instant::now();
        // Ceci va bloquer (await) tant que lock1 n'est pas lâché
        let _guard = lock2.write().await;
        let duration = start.elapsed();

        handle.await.unwrap();

        // Le verrou fonctionne si on a dû attendre au moins ~50ms
        assert!(duration >= Duration::from_millis(50));
    }
}
