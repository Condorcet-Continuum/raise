// src-tauri/src/blockchain/p2p/protocol.rs

use crate::blockchain::consensus::vote::Vote; // Ajout nécessaire
use crate::blockchain::storage::commit::ArcadiaCommit;
use serde::{Deserialize, Serialize};

/// Types de messages échangés sur le réseau P2P souverain.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum ArcadiaNetMessage {
    /// Diffusion d'un nouveau commit (Gossip).
    AnnounceCommit(ArcadiaCommit),

    /// Diffusion d'un vote individuel pour un commit (Gossip).
    /// Permet aux validateurs de collecter les signatures pour atteindre le quorum.
    SubmitVote(Vote),

    /// Demande d'un commit spécifique par son hash (Req/Resp).
    RequestCommit { commit_hash: String },

    /// Requête pour obtenir le dernier hash connu d'un pair.
    RequestLatestHash,
}

/// Réponses possibles aux requêtes directes.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum ArcadiaResponse {
    /// Retourne le commit demandé.
    CommitFound(ArcadiaCommit),

    /// Indique que le commit est inconnu.
    CommitNotFound,

    /// Retourne le dernier hash de la chaîne.
    LatestHash(Option<String>),
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn test_protocol_serialization() {
        let msg = ArcadiaNetMessage::RequestLatestHash;
        let serialized = serde_json::to_string(&msg).unwrap();
        assert!(serialized.contains("RequestLatestHash"));
    }

    #[test]
    fn test_vote_serialization() {
        let vote = Vote {
            commit_id: "test_hash".into(),
            validator_key: "validator_1".into(),
            signature: vec![0, 1, 2, 3],
        };

        let msg = ArcadiaNetMessage::SubmitVote(vote);
        let serialized = serde_json::to_string(&msg).unwrap();

        assert!(serialized.contains("SubmitVote"));
        assert!(serialized.contains("validator_1"));
    }

    #[test]
    fn test_commit_announcement_serialization() {
        let commit = ArcadiaCommit {
            id: "commit_1".into(),
            parent_hash: None,
            author: "author_1".into(),
            timestamp: Utc::now(),
            mutations: vec![],
            merkle_root: "root".into(),
            signature: vec![],
        };

        let msg = ArcadiaNetMessage::AnnounceCommit(commit);
        let serialized = serde_json::to_string(&msg).unwrap();
        assert!(serialized.contains("AnnounceCommit"));
    }
}
