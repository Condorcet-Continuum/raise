// src-tauri/src/blockchain/consensus/mod.rs
//! Consensus Mentis : Orchestration de la validation collective des mutations.

pub mod leader;
pub mod pending;
pub mod vote;

use crate::blockchain::consensus::vote::{Vote, VoteCollector};
use crate::blockchain::storage::commit::MentisCommit;
use crate::utils::prelude::*;

/// Moteur de consensus gérant les cycles de validation des blocs.
pub struct ConsensusEngine {
    pub pending_validations: UnorderedMap<String, VoteCollector>,
    pub default_quorum: usize,
}

impl ConsensusEngine {
    /// Initialise un nouveau moteur de consensus avec un quorum par défaut.
    pub fn new(default_quorum: usize) -> Self {
        Self {
            pending_validations: UnorderedMap::new(),
            default_quorum,
        }
    }

    /// Enregistre un nouveau commit en attente de validation.
    pub fn register_commit(&mut self, commit: &MentisCommit) {
        if !self.pending_validations.contains_key(&commit.id) {
            self.pending_validations.insert(
                commit.id.clone(),
                VoteCollector::new(commit.id.clone(), self.default_quorum),
            );
            user_trace!(
                "TRC_CONSENSUS_REGISTER",
                json_value!({ "commit_id": commit.id, "quorum_required": self.default_quorum })
            );
        }
    }

    /// Traite un vote entrant et vérifie si le quorum est atteint.
    /// Retourne `true` si le bloc vient d'atteindre le quorum de validation.
    pub fn process_incoming_vote(&mut self, vote: Vote) -> bool {
        if let Some(collector) = self.pending_validations.get_mut(&vote.commit_id) {
            // On ajoute le vote (add_vote gère la vérification cryptographique et l'Anti-Sybil)
            if collector.add_vote(&vote) {
                let is_validated = collector.is_validated();

                if is_validated {
                    user_success!(
                        "INF_CONSENSUS_REACHED",
                        json_value!({ "commit_id": vote.commit_id })
                    );
                }
                return is_validated;
            }
        }
        false
    }

    /// Nettoie les validations en attente trop anciennes pour éviter les fuites de mémoire.
    pub fn garbage_collect(&mut self, max_age_minutes: i64) {
        let now = UtcClock::now();
        let initial_count = self.pending_validations.len();

        self.pending_validations
            .retain(|_, collector| (now - collector.created_at).num_minutes() < max_age_minutes);

        let removed = initial_count - self.pending_validations.len();
        if removed > 0 {
            user_trace!(
                "TRC_CONSENSUS_GC",
                json_value!({ "purged_collectors": removed })
            );
        }
    }

    /// Finalise un cycle de validation en retirant le collecteur de la mémoire.
    /// Typiquement appelé après que le bloc ait été persisté sur le disque.
    pub fn finalize_validation(&mut self, commit_id: &str) {
        if self.pending_validations.remove(commit_id).is_some() {
            user_trace!(
                "TRC_CONSENSUS_FINALIZED",
                json_value!({ "commit_id": commit_id })
            );
        }
    }
}

// =========================================================================
// TESTS UNITAIRES (Audit du Moteur de Consensus)
// =========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blockchain::crypto::signing::KeyPair;

    #[test]
    fn test_consensus_full_lifecycle() {
        let keys_auth = KeyPair::generate();
        let keys_v1 = KeyPair::generate();

        let mut engine = ConsensusEngine::new(2);
        let commit = MentisCommit::new(vec![], None, &keys_auth);

        // 1. Enregistrement
        engine.register_commit(&commit);
        assert!(engine.pending_validations.contains_key(&commit.id));

        // 2. Premier vote par l'API officielle (Quorum non atteint)
        let vote1 = Vote::new(commit.id.clone(), &keys_v1);
        assert!(!engine.process_incoming_vote(vote1));

        // 3. Second vote simulé en contournant KeyPair::generate() (Anti-Sybil)
        // On récupère le collecteur et on injecte un votant factice
        if let Some(collector) = engine.pending_validations.get_mut(&commit.id) {
            collector.voters.insert("mock_voter_2".to_string());

            // On vérifie que la mécanique de quorum de l'Engine fonctionne
            let is_validated = collector.is_validated();
            assert!(
                is_validated,
                "Le moteur doit valider que le quorum de 2 est atteint"
            );
        } else {
            panic!("Le collecteur a disparu");
        }

        // 4. Finalisation (Purge)
        engine.finalize_validation(&commit.id);
        assert!(
            !engine.pending_validations.contains_key(&commit.id),
            "Le collecteur doit être supprimé après finalisation"
        );
    }

    #[test]
    fn test_consensus_ignore_unregistered_id() {
        let keys = KeyPair::generate();
        let mut engine = ConsensusEngine::new(1);

        // On tente de voter pour un bloc qui n'a pas été enregistré
        let ghost_vote = Vote::new("ghost_id".into(), &keys);

        assert!(
            !engine.process_incoming_vote(ghost_vote),
            "Le vote pour un ID non enregistré doit être ignoré"
        );
    }

    #[test]
    fn test_consensus_garbage_collection() {
        let keys = KeyPair::generate();
        let mut engine = ConsensusEngine::new(2);

        let commit = MentisCommit::new(vec![], None, &keys);
        engine.register_commit(&commit);

        // On modifie manuellement la date de création du collecteur pour simuler le temps qui passe
        if let Some(collector) = engine.pending_validations.get_mut(&commit.id) {
            collector.created_at = UtcClock::now() - TimeDuration::from_secs(60 * 60);
            // Il y a 1 heure
        }

        // On lance le GC pour tout ce qui a plus de 30 minutes
        engine.garbage_collect(30);

        assert!(
            !engine.pending_validations.contains_key(&commit.id),
            "Le vieux collecteur aurait dû être purgé par le GC"
        );
    }
}
