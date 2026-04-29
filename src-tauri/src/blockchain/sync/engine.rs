// src-tauri/src/blockchain/sync/engine.rs
//! Moteur de synchronisation Mentis : Répond aux requêtes de synchronisation des autres nœuds.

use crate::blockchain::p2p::protocol::{MentisNetMessage, MentisResponse};
use crate::blockchain::storage::chain::Ledger;
use crate::utils::prelude::*;

/// Le moteur de synchronisation Mentis.
pub struct SyncEngine {
    /// Référence partagée vers le registre local (Ledger).
    ledger: SharedRef<SyncMutex<Ledger>>,
}

impl SyncEngine {
    /// Crée une nouvelle instance du moteur de synchronisation.
    pub fn new(ledger: SharedRef<SyncMutex<Ledger>>) -> Self {
        Self { ledger }
    }

    /// Traite une requête de synchronisation ciblée et génère la réponse MentisResponse.
    pub fn process_sync_request(
        &self,
        req: &MentisNetMessage,
    ) -> RaiseResult<Option<MentisResponse>> {
        match req {
            // 🔍 Un pair demande quel est notre dernier hash
            MentisNetMessage::RequestLatestHash => {
                let guard = match self.ledger.lock() {
                    Ok(g) => g,
                    Err(_) => raise_error!("ERR_SYNC_LEDGER_LOCK", error = "Ledger lock poisoned"),
                };

                user_trace!(
                    "TRC_SYNC_LATEST_HASH",
                    json_value!({ "latest": guard.last_commit_hash })
                );
                Ok(Some(MentisResponse::LatestHash(
                    guard.last_commit_hash.clone(),
                )))
            }

            // 📥 Un pair demande un commit spécifique qu'il lui manque
            MentisNetMessage::RequestCommit { commit_hash } => {
                let _guard = match self.ledger.lock() {
                    Ok(g) => g,
                    Err(_) => raise_error!("ERR_SYNC_LEDGER_LOCK", error = "Ledger lock poisoned"),
                };

                user_trace!("TRC_SYNC_GET_COMMIT", json_value!({ "hash": commit_hash }));

                // TODO: Appeler guard.get_commit(commit_hash) quand l'API Ledger le permettra.
                // Pour l'instant, on simule que le commit n'est pas trouvé.
                Ok(Some(MentisResponse::CommitNotFound))
            }

            // Les messages de diffusion (AnnounceCommit, SubmitVote) sont ignorés ici,
            // ils sont traités en amont par le ConsensusEngine dans service.rs.
            _ => Ok(None),
        }
    }
}

// =========================================================================
// TESTS UNITAIRES (Audit du Moteur de Synchronisation)
// =========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sync_engine_request_latest_hash() {
        let ledger = SharedRef::new(SyncMutex::new(Ledger::new()));
        let engine = SyncEngine::new(ledger.clone());

        let req = MentisNetMessage::RequestLatestHash;
        let response = engine.process_sync_request(&req).unwrap();

        // Un ledger neuf n'a pas de hash de tête
        assert_eq!(response, Some(MentisResponse::LatestHash(None)));
    }

    #[test]
    fn test_sync_engine_request_commit() {
        let ledger = SharedRef::new(SyncMutex::new(Ledger::new()));
        let engine = SyncEngine::new(ledger);

        let req = MentisNetMessage::RequestCommit {
            commit_hash: "123".into(),
        };
        let response = engine.process_sync_request(&req).unwrap();

        // Par défaut, le mock renvoie CommitNotFound
        assert_eq!(response, Some(MentisResponse::CommitNotFound));
    }

    #[test]
    fn test_sync_engine_ignores_gossip() {
        let ledger = SharedRef::new(SyncMutex::new(Ledger::new()));
        let engine = SyncEngine::new(ledger);

        // Un message SubmitVote ne doit pas générer de réponse de synchronisation
        let vote = crate::blockchain::consensus::vote::Vote {
            commit_id: "abc".into(),
            voter: "xyz".into(),
            signature: vec![],
        };
        let req = MentisNetMessage::SubmitVote(vote);

        let response = engine.process_sync_request(&req).unwrap();
        assert_eq!(
            response, None,
            "Les messages Gossip doivent être ignorés par le SyncEngine"
        );
    }
}
