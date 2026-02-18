// src-tauri/src/blockchain/consensus/pending.rs

use crate::blockchain::storage::commit::ArcadiaCommit;
use crate::utils::{DateTime, HashMap, Utc};
/// Représente un commit en attente avec sa date de réception pour gérer l'expiration.
pub struct PendingEntry {
    pub commit: ArcadiaCommit,
    pub received_at: DateTime<Utc>,
}

/// Gestionnaire des commits en attente de validation par quorum.
pub struct PendingCommits {
    entries: HashMap<String, PendingEntry>,
}

impl PendingCommits {
    /// Crée un nouveau gestionnaire de commits en attente.
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    /// Ajoute ou met à jour un commit en attente.
    pub fn insert(&mut self, commit: ArcadiaCommit) {
        let id = commit.id.clone();
        self.entries.insert(
            id,
            PendingEntry {
                commit,
                received_at: Utc::now(),
            },
        );
    }

    /// Récupère un commit par son ID sans le retirer.
    pub fn get(&self, commit_id: &str) -> Option<&ArcadiaCommit> {
        self.entries.get(commit_id).map(|e| &e.commit)
    }

    /// Supprime et retourne le commit (typiquement après obtention du quorum).
    pub fn remove(&mut self, commit_id: &str) -> Option<ArcadiaCommit> {
        self.entries.remove(commit_id).map(|e| e.commit)
    }

    /// Nettoie les commits trop vieux pour libérer la mémoire.
    pub fn garbage_collect(&mut self, max_age_minutes: i64) {
        let now = Utc::now();
        self.entries
            .retain(|_, entry| (now - entry.received_at).num_minutes() < max_age_minutes);
    }
}

/// Implémentation de Default requise par Clippy pour les types avec new().
impl Default for PendingCommits {
    fn default() -> Self {
        Self::new()
    }
}

// --- TESTS UNITAIRES ---

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    fn mock_commit(id: &str) -> ArcadiaCommit {
        ArcadiaCommit {
            id: id.into(),
            parent_hash: None,
            author: "key".into(),
            timestamp: Utc::now(),
            mutations: vec![],
            merkle_root: "root".into(),
            signature: vec![],
        }
    }

    #[test]
    fn test_pending_storage_flow() {
        let mut pending = PendingCommits::new();
        let commit = mock_commit("tx_1");

        pending.insert(commit.clone());
        assert!(pending.get("tx_1").is_some());

        let removed = pending.remove("tx_1").expect("Devrait trouver le commit");
        assert_eq!(removed.id, "tx_1");
        assert!(pending.get("tx_1").is_none());
    }

    #[test]
    fn test_garbage_collection() {
        let mut pending = PendingCommits::new();
        let id = "old_tx";

        // Simulation d'un commit reçu il y a 40 minutes
        pending.entries.insert(
            id.into(),
            PendingEntry {
                commit: mock_commit(id),
                received_at: Utc::now() - Duration::minutes(40),
            },
        );

        pending.garbage_collect(30);
        assert!(
            pending.get(id).is_none(),
            "Le vieux commit aurait dû être supprimé"
        );
    }

    #[test]
    fn test_default_impl() {
        let pending = PendingCommits::default();
        assert!(pending.get("any").is_none());
    }
}
