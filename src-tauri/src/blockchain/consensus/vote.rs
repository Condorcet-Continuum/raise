// src-tauri/src/blockchain/consensus/vote.rs
//! Gestion du mécanisme de vote et de la validation du quorum pour Arcadia.

use crate::utils::{prelude::*, HashMap};

/// Représente un vote individuel émis par un validateur.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Vote {
    /// Identifiant du commit (hash) sur lequel porte le vote.
    pub commit_id: String,
    /// Clé publique du validateur.
    pub validator_key: String,
    /// Signature cryptographique du commit par le validateur.
    pub signature: Vec<u8>,
}

/// Collecteur de votes pour un commit spécifique.
pub struct VoteCollector {
    /// Mapping : commit_id -> (Mapping : validator_key -> Vote)
    votes: HashMap<String, HashMap<String, Vote>>,
    /// Taille du quorum requise (définie par le ConsensusConfig).
    required_quorum: usize,
}

impl VoteCollector {
    /// Crée un nouveau collecteur avec le quorum spécifié.
    pub fn new(required_quorum: usize) -> Self {
        Self {
            votes: HashMap::new(),
            required_quorum,
        }
    }

    /// Ajoute un vote au collecteur.
    /// Retourne true si le quorum est atteint après cet ajout.
    pub fn add_vote(&mut self, vote: Vote) -> bool {
        // CORRECTION CLIPPY : Utilisation de or_default() au lieu de or_insert_with(HashMap::new)
        let commit_votes = self.votes.entry(vote.commit_id.clone()).or_default();

        commit_votes.insert(vote.validator_key.clone(), vote);

        commit_votes.len() >= self.required_quorum
    }

    /// Vérifie si un commit a atteint le quorum.
    pub fn has_quorum(&self, commit_id: &str) -> bool {
        if let Some(commit_votes) = self.votes.get(commit_id) {
            return commit_votes.len() >= self.required_quorum;
        }
        false
    }

    /// Récupère tous les votes pour un commit (utile pour l'agrégation finale).
    pub fn get_votes(&self, commit_id: &str) -> Option<&HashMap<String, Vote>> {
        self.votes.get(commit_id)
    }

    /// Nettoie les votes d'un commit une fois validé ou expiré.
    pub fn clear_commit(&mut self, commit_id: &str) {
        self.votes.remove(commit_id);
    }
}

impl Default for VoteCollector {
    fn default() -> Self {
        Self::new(1) // Par défaut, au moins une signature
    }
}

// --- TESTS UNITAIRES ---

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vote_collection_and_quorum() {
        // Quorum de 2 requis
        let mut collector = VoteCollector::new(2);
        let commit_id = "hash_123".to_string();

        let v1 = Vote {
            commit_id: commit_id.clone(),
            validator_key: "pub_key_A".into(),
            signature: vec![1, 2, 3],
        };

        let v2 = Vote {
            commit_id: commit_id.clone(),
            validator_key: "pub_key_B".into(),
            signature: vec![4, 5, 6],
        };

        // Premier vote : quorum non atteint
        assert!(!collector.add_vote(v1));
        assert!(!collector.has_quorum(&commit_id));

        // Deuxième vote (autre validateur) : quorum atteint
        assert!(collector.add_vote(v2));
        assert!(collector.has_quorum(&commit_id));
    }

    #[test]
    fn test_clear_votes() {
        let mut collector = VoteCollector::new(1);
        let v = Vote {
            commit_id: "hash".into(),
            validator_key: "key".into(),
            signature: vec![],
        };
        collector.add_vote(v);
        assert!(collector.has_quorum("hash"));

        collector.clear_commit("hash");
        assert!(!collector.has_quorum("hash"));
    }
}
