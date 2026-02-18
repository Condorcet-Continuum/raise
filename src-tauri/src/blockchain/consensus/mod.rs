// src-tauri/src/blockchain/consensus/mod.rs

use crate::blockchain::storage::commit::ArcadiaCommit;
use crate::blockchain::vpn::innernet_client::Peer;
use crate::utils::{prelude::*, HashSet};
// Déclaration des sous-modules
pub mod leader;
pub mod pending;
pub mod vote; // Nouveau module intégré

// Réexportations
pub use leader::LeaderElection;
pub use pending::PendingCommits;
pub use vote::{Vote, VoteCollector};

/// Alias de compatibilité pour le reste du projet
pub type ArcadiaConsensus = ConsensusEngine;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ConsensusConfig {
    pub authorized_validators: HashSet<String>,
    pub required_quorum: usize,
}

/// Moteur de consensus centralisant la validation des autorités et des quorums.
pub struct ConsensusEngine {
    config: ConsensusConfig,
    collector: VoteCollector,
    pending: PendingCommits, // Stockage dédié pour les commits non encore validés
}

impl ConsensusEngine {
    pub fn new(peers: &[Peer], quorum_ratio: f32) -> Self {
        let validators: HashSet<String> = peers.iter().map(|p| p.public_key.clone()).collect();
        let threshold = ((validators.len() as f32) * quorum_ratio).ceil() as usize;
        let quorum = threshold.max(1);

        Self {
            config: ConsensusConfig {
                authorized_validators: validators,
                required_quorum: quorum,
            },
            collector: VoteCollector::new(quorum),
            pending: PendingCommits::new(),
        }
    }

    /// Méthode de compatibilité
    pub fn from_vpn_peers(peers: &[Peer], quorum_ratio: f32) -> Self {
        Self::new(peers, quorum_ratio)
    }

    /// Vérifie l'autorité et mémorise le commit s'il est valide.
    pub fn register_proposal(&mut self, commit: ArcadiaCommit) -> Result<()> {
        if !self.config.authorized_validators.contains(&commit.author) {
            return Err(AppError::Validation(format!(
                "Commit rejeté : auteur {} non autorisé",
                commit.author
            )));
        }

        // Stockage temporaire en attendant les votes
        self.pending.insert(commit);
        Ok(())
    }

    /// Enregistre un vote. Si le quorum est atteint, retourne le commit finalisé.
    pub fn process_vote(&mut self, vote: Vote) -> Result<Option<ArcadiaCommit>> {
        if !self
            .config
            .authorized_validators
            .contains(&vote.validator_key)
        {
            return Err(AppError::Validation(format!(
                "Vote rejeté : validateur {} non autorisé",
                vote.validator_key
            )));
        }

        if self.collector.add_vote(vote.clone()) {
            // Le quorum est atteint, on extrait le commit pour le Bridge
            Ok(self.pending.remove(&vote.commit_id))
        } else {
            Ok(None)
        }
    }

    /// Alias de compatibilité
    pub fn verify_authority(&self, commit: &ArcadiaCommit) -> bool {
        self.config.authorized_validators.contains(&commit.author)
    }

    pub fn finalize_commit(&mut self, commit_id: &str) {
        self.collector.clear_commit(commit_id);
        self.pending.remove(commit_id);
    }

    pub fn get_quorum_size(&self) -> usize {
        self.config.required_quorum
    }
}

// --- TESTS UNITAIRES ---

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::Utc;

    fn mock_peer(key: &str) -> Peer {
        Peer {
            name: "test".into(),
            public_key: key.into(),
            ip: "10.42.0.1".into(),
            endpoint: None,
            last_handshake: None,
            transfer_rx: 0,
            transfer_tx: 0,
        }
    }

    #[test]
    fn test_consensus_full_cycle() {
        let peers = vec![mock_peer("key1"), mock_peer("key2")];
        let mut engine = ConsensusEngine::new(&peers, 1.0); // 2 votes requis

        let commit = ArcadiaCommit {
            id: "tx1".into(),
            parent_hash: None,
            author: "key1".into(),
            timestamp: Utc::now(),
            mutations: vec![],
            merkle_root: "root".into(),
            signature: vec![],
        };

        // 1. Enregistrement du commit
        engine.register_proposal(commit).unwrap();

        // 2. Premier vote
        let res1 = engine
            .process_vote(Vote {
                commit_id: "tx1".into(),
                validator_key: "key1".into(),
                signature: vec![1],
            })
            .unwrap();
        assert!(res1.is_none());

        // 3. Deuxième vote -> Finalisation
        let res2 = engine
            .process_vote(Vote {
                commit_id: "tx1".into(),
                validator_key: "key2".into(),
                signature: vec![2],
            })
            .unwrap();

        assert!(res2.is_some());
        assert_eq!(res2.unwrap().id, "tx1");
    }
}
