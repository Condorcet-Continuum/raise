// src-tauri/src/blockchain/sync/engine.rs

use crate::blockchain::p2p::protocol::ArcadiaNetMessage;
use crate::blockchain::storage::chain::Ledger;
use crate::blockchain::sync::state::SyncStatus;

/// Moteur de réconciliation de la chaîne Arcadia.
pub struct SyncEngine {
    pub status: SyncStatus,
}

impl Default for SyncEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl SyncEngine {
    /// Initialise le moteur de synchronisation dans l'état Initializing.
    pub fn new() -> Self {
        Self {
            status: SyncStatus::Initializing,
        }
    }

    /// Compare notre dernier commit avec celui d'un pair pour décider de la marche à suivre.
    /// Retourne une requête réseau si une synchronisation est nécessaire.
    pub fn reconcile(
        &mut self,
        local_ledger: &Ledger,
        remote_last_hash: Option<String>,
    ) -> Option<ArcadiaNetMessage> {
        let local_hash = local_ledger.last_commit_hash.clone();

        // Cas 1 : Les chaînes sont identiques (ou les deux sont vides)
        if remote_last_hash == local_hash {
            self.status = SyncStatus::UpToDate;
            return None;
        }

        // Cas 2 : Le pair distant a une tête de chaîne différente
        match remote_last_hash {
            Some(hash) => {
                // Le distant a un commit que nous n'avons pas ou est sur un fork.
                // On passe en état Syncing et on demande le commit manquant.
                self.status = SyncStatus::Syncing {
                    progress: 0.0,
                    target_hash: hash.clone(),
                };
                Some(ArcadiaNetMessage::RequestCommit { commit_hash: hash })
            }
            None => {
                // Le distant est vide alors que nous avons des données (ou inversement géré par Cas 1).
                // On reste Idle ou UpToDate par rapport à ce pair.
                self.status = SyncStatus::UpToDate;
                None
            }
        }
    }

    /// Met à jour l'état de synchronisation manuellement.
    pub fn set_status(&mut self, new_status: SyncStatus) {
        self.status = new_status;
    }
}

// --- TESTS UNITAIRES ---

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blockchain::storage::chain::Ledger;

    #[test]
    fn test_sync_reconcile_up_to_date() {
        let mut engine = SyncEngine::new();
        let mut ledger = Ledger::new();
        ledger.last_commit_hash = Some("hash1".to_string());

        let decision = engine.reconcile(&ledger, Some("hash1".to_string()));

        assert!(decision.is_none());
        assert_eq!(engine.status, SyncStatus::UpToDate);
    }

    #[test]
    fn test_sync_request_missing_commit() {
        let mut engine = SyncEngine::new();
        let ledger = Ledger::new(); // Ledger vide

        let decision = engine.reconcile(&ledger, Some("remote_hash".to_string()));

        if let Some(ArcadiaNetMessage::RequestCommit { commit_hash }) = decision {
            assert_eq!(commit_hash, "remote_hash");
        } else {
            panic!("Le moteur de synchronisation devrait générer une RequestCommit");
        }
    }

    #[test]
    fn test_sync_with_empty_remote() {
        let mut engine = SyncEngine::new();
        let mut ledger = Ledger::new();
        ledger.last_commit_hash = Some("local_hash".to_string());

        // Le pair distant n'a rien (None)
        let decision = engine.reconcile(&ledger, None);

        assert!(decision.is_none());
        assert_eq!(engine.status, SyncStatus::UpToDate);
    }
}
