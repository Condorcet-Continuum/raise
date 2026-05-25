// src-tauri/src/blockchain/p2p/protocol.rs
//! Protocole réseau Mentis : Définition unifiée des messages du réseau P2P.

use crate::blockchain::consensus::vote::Vote;
use crate::blockchain::storage::commit::MentisCommit;
use crate::utils::prelude::*;

/// 🛰️ Messages du réseau Mentis.
/// Regroupe la diffusion (Gossipsub) et les requêtes de synchronisation (Request-Response).
#[derive(Debug, Serializable, Deserializable, Clone, PartialEq)]
pub enum MentisNetMessage {
    /// Annonce d'un nouveau bloc de connaissance (Diffusion globale).
    AnnounceCommit(MentisCommit),

    /// Diffusion d'un vote de validation pour atteindre le quorum (Consensus).
    SubmitVote(Vote),

    /// Requête ciblée pour obtenir un bloc spécifique manquant (Synchronisation).
    RequestCommit { commit_hash: String },

    /// Requête pour obtenir le hash de tête (Head) d'un pair afin de vérifier la synchro.
    RequestLatestHash,
}

/// 📦 Réponses directes du protocole Mentis.
/// Utilisé exclusivement dans les échanges ciblés (Request-Response) pour le transfert de données.
#[derive(Debug, Serializable, Deserializable, Clone, PartialEq)]
pub enum MentisResponse {
    /// Retourne le commit complet demandé par RequestCommit.
    CommitFound(MentisCommit),

    /// Indique que le commit demandé est inconnu sur le nœud local.
    CommitNotFound,

    /// Retourne le hash de tête du Ledger local.
    LatestHash(Option<String>),

    /// Acquittement simple (pour des requêtes ne nécessitant pas de payload de retour).
    Ack,
}

// =========================================================================
// TESTS UNITAIRES (Audit du Protocole de Sérialisation)
// =========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blockchain::crypto::signing::KeyPair;
    use crate::blockchain::storage::commit::{Mutation, MutationOp};

    /// Test 1 : Roundtrip de sérialisation pour les annonces de commits.
    #[test]
    fn test_protocol_announce_serialization() {
        let keys = KeyPair::generate();
        let commit = MentisCommit::new(vec![], None, &keys);
        let msg = MentisNetMessage::AnnounceCommit(commit.clone());

        let encoded = json::serialize_to_string(&msg).expect("Échec de la sérialisation");
        let decoded: MentisNetMessage =
            json::deserialize_from_str(&encoded).expect("Échec de la désérialisation");

        assert_eq!(
            msg, decoded,
            "Le message AnnounceCommit doit survivre au cycle JSON à l'identique."
        );
    }

    /// Test 2 : Roundtrip pour les requêtes de synchronisation (variants structurels).
    #[test]
    fn test_protocol_request_serialization() {
        let msg = MentisNetMessage::RequestCommit {
            commit_hash: "target_hash_123".into(),
        };

        let encoded = json::serialize_to_string(&msg).unwrap();
        let decoded: MentisNetMessage = json::deserialize_from_str(&encoded).unwrap();

        assert_eq!(msg, decoded);
    }

    /// Test 3 : Robustesse des réponses avec Option (LatestHash).
    #[test]
    fn test_protocol_response_latest_hash() {
        let res_some = MentisResponse::LatestHash(Some("head_hash".into()));
        let res_none = MentisResponse::LatestHash(None);

        let encoded_some = json::serialize_to_string(&res_some).unwrap();
        let decoded_some: MentisResponse = json::deserialize_from_str(&encoded_some).unwrap();
        assert_eq!(res_some, decoded_some);

        let encoded_none = json::serialize_to_string(&res_none).unwrap();
        let decoded_none: MentisResponse = json::deserialize_from_str(&encoded_none).unwrap();
        assert_eq!(res_none, decoded_none);
    }

    /// Test 4 : Stress test de message massif (Gros payload de mutations).
    #[test]
    fn test_protocol_large_message() {
        let keys = KeyPair::generate();
        let mut mutations = Vec::new();
        for i in 0..100 {
            mutations.push(Mutation {
                element_id: format!("urn:test:{}", i),
                operation: MutationOp::Create,
                payload: json_value!({"index": i, "data": "dummy_content"}),
            });
        }

        let commit = MentisCommit::new(mutations, None, &keys);
        let msg = MentisNetMessage::AnnounceCommit(commit);

        let encoded = json::serialize_to_string(&msg).unwrap();
        let decoded: MentisNetMessage = json::deserialize_from_str(&encoded).unwrap();

        if let MentisNetMessage::AnnounceCommit(c) = decoded {
            assert_eq!(c.mutations.len(), 100);
        } else {
            panic!("Mauvais type de message après décodage");
        }
    }
}
