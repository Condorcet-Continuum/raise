// src-tauri/src/blockchain/consensus/pending.rs
//! Gestion de la mémoire tampon des commits en attente de validation.

use crate::blockchain::storage::commit::MentisCommit;
use crate::utils::prelude::*;

/// Représente un commit en attente avec sa date de réception pour gérer l'expiration.
#[derive(Debug, Clone)]
pub struct PendingEntry {
    pub commit: MentisCommit,
    pub received_at: UtcTimestamp,
}

/// Gestionnaire des commits en attente de validation par quorum.
#[derive(Debug, Clone)]
pub struct PendingCommits {
    entries: UnorderedMap<String, PendingEntry>,
}

impl PendingCommits {
    /// Crée un nouveau gestionnaire de commits en attente.
    pub fn new() -> Self {
        Self {
            entries: UnorderedMap::new(),
        }
    }

    /// Ajoute ou met à jour un commit en attente.
    pub fn insert(&mut self, commit: MentisCommit) {
        let id = commit.id.clone();

        // On insère (ou met à jour la date de réception si déjà présent)
        self.entries.insert(
            id.clone(),
            PendingEntry {
                commit,
                received_at: UtcClock::now(),
            },
        );

        user_trace!(
            "TRC_PENDING_COMMIT_INSERTED",
            json_value!({ "commit_id": id, "action": "buffer_pending" })
        );
    }

    /// Récupère un commit par son ID sans le retirer.
    pub fn get(&self, commit_id: &str) -> Option<&MentisCommit> {
        self.entries.get(commit_id).map(|e| &e.commit)
    }

    /// Supprime et retourne le commit (typiquement après obtention du quorum ou rejet).
    pub fn remove(&mut self, commit_id: &str) -> Option<MentisCommit> {
        let removed = self.entries.remove(commit_id).map(|e| e.commit);

        if removed.is_some() {
            user_trace!(
                "TRC_PENDING_COMMIT_REMOVED",
                json_value!({ "commit_id": commit_id })
            );
        }

        removed
    }

    /// Nettoie les commits trop vieux pour libérer la mémoire (Garbage Collection).
    pub fn garbage_collect(&mut self, max_age_minutes: i64) {
        let now = UtcClock::now();
        let initial_count = self.entries.len();

        self.entries
            .retain(|_, entry| (now - entry.received_at).num_minutes() < max_age_minutes);

        let removed = initial_count - self.entries.len();
        if removed > 0 {
            user_trace!(
                "TRC_PENDING_GC_PURGED",
                json_value!({ "purged_commits": removed })
            );
        }
    }
}

/// Implémentation de Default requise par Clippy pour les constructeurs sans arguments.
impl Default for PendingCommits {
    fn default() -> Self {
        Self::new()
    }
}

// =========================================================================
// TESTS UNITAIRES (Audit du Buffer en Mémoire)
// =========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper pour générer un commit factice ultra-léger pour les tests
    fn mock_commit(id: &str) -> MentisCommit {
        MentisCommit {
            id: id.into(),
            parent_hash: None,
            author: "mock_author".into(),
            timestamp: UtcClock::now(),
            mutations: vec![],
            merkle_root: "mock_root".into(),
            signature: vec![], // Pas besoin de vraie signature ici, on teste juste le stockage
        }
    }

    #[test]
    fn test_pending_storage_flow() {
        let mut pending = PendingCommits::new();
        let commit = mock_commit("tx_1");

        // 1. Insertion
        pending.insert(commit.clone());
        assert!(
            pending.get("tx_1").is_some(),
            "Le commit devrait être trouvé dans le buffer"
        );

        // 2. Suppression et récupération
        let removed = pending
            .remove("tx_1")
            .expect("Devrait retourner le commit supprimé");
        assert_eq!(removed.id, "tx_1");
        assert!(
            pending.get("tx_1").is_none(),
            "Le commit devrait être absent après suppression"
        );
    }

    #[test]
    fn test_garbage_collection() {
        let mut pending = PendingCommits::new();
        let old_id = "old_tx";
        let new_id = "new_tx";

        // Simulation d'un vieux commit reçu il y a 40 minutes
        pending.entries.insert(
            old_id.into(),
            PendingEntry {
                commit: mock_commit(old_id),
                received_at: UtcClock::now() - TimeDuration::from_secs(40 * 60),
            },
        );

        // Simulation d'un commit récent reçu à l'instant
        pending.entries.insert(
            new_id.into(),
            PendingEntry {
                commit: mock_commit(new_id),
                received_at: UtcClock::now(),
            },
        );

        // On lance le Garbage Collector pour purger les éléments de plus de 30 minutes
        pending.garbage_collect(30);

        // Vérifications de l'isolation du nettoyage
        assert!(
            pending.get(old_id).is_none(),
            "Le vieux commit aurait dû être supprimé par le GC"
        );
        assert!(
            pending.get(new_id).is_some(),
            "Le commit récent DOIT être conservé par le GC"
        );
    }

    #[test]
    fn test_default_impl() {
        let pending = PendingCommits::default();
        assert!(pending.get("ghost").is_none());
    }
}
