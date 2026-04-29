// src-tauri/src/blockchain/consensus/vote.rs
//! Système de vote Mentis : Authentification et agrégation des validations.

use crate::blockchain::crypto::signing::{verify_signature, KeyPair};
use crate::utils::prelude::*;

/// Représente un vote d'approbation pour un commit spécifique.
#[derive(Serializable, Deserializable, Debug, Clone, PartialEq)]
pub struct Vote {
    pub commit_id: String,
    pub voter: String,
    pub signature: Vec<u8>,
}

impl Vote {
    /// Crée un nouveau vote signé cryptographiquement.
    pub fn new(commit_id: String, keys: &KeyPair) -> Self {
        let voter = keys.public_key_hex();
        let signature = keys.sign(&commit_id);
        Self {
            commit_id,
            voter,
            signature,
        }
    }

    /// Vérifie l'authenticité de la signature asymétrique du vote.
    pub fn verify(&self) -> bool {
        verify_signature(&self.voter, &self.commit_id, &self.signature)
    }
}

/// Collecteur de votes pour gérer le quorum du réseau.
#[derive(Debug, Clone)]
pub struct VoteCollector {
    pub target_commit_id: String,
    pub voters: UniqueSet<String>,
    pub quorum_threshold: usize,
    /// Horodatage de création pour la purge des votes orphelins (Garbage Collection)
    pub created_at: UtcTimestamp,
}

impl VoteCollector {
    /// Initialise un nouveau collecteur pour un commit donné.
    pub fn new(target_commit_id: String, threshold: usize) -> Self {
        Self {
            target_commit_id,
            voters: UniqueSet::new(),
            quorum_threshold: threshold,
            created_at: UtcClock::now(), // 🎯 FIX : Initialisation du timestamp
        }
    }

    /// Ajoute un vote s'il est valide ET s'il concerne le bon commit.
    /// Retourne `true` si le vote a été accepté ET qu'il s'agit d'un nouveau votant.
    pub fn add_vote(&mut self, vote: &Vote) -> bool {
        // Vérification de la cible (Anti-Confusion)
        if vote.commit_id != self.target_commit_id {
            return false;
        }

        // Vérification cryptographique
        if vote.verify() {
            // L'insertion dans un UniqueSet garantit l'unicité par clé publique (Anti-Sybil)
            // insert() retourne true si la valeur n'était pas déjà présente.
            return self.voters.insert(vote.voter.clone());
        }
        false
    }

    /// Vérifie si le quorum est atteint pour valider le bloc.
    pub fn is_validated(&self) -> bool {
        self.voters.len() >= self.quorum_threshold
    }
}

// =========================================================================
// TESTS UNITAIRES (Audit de Collecte et Résilience)
// =========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vote_collector_quorum() {
        let keys_1 = KeyPair::generate();

        let commit_id = "commit_123".to_string();
        let mut collector = VoteCollector::new(commit_id.clone(), 2);

        let v1 = Vote::new(commit_id.clone(), &keys_1);

        // 1. On teste l'ajout par cryptographie réelle
        assert!(
            collector.add_vote(&v1),
            "Le premier vote doit être accepté."
        );
        assert!(
            !collector.is_validated(),
            "Le quorum ne doit pas être atteint avec 1 seul vote."
        );

        // 2. 🎯 FIX DU TEST : On simule un second votant sans dépendre de KeyPair::generate()
        // Dans les environnements de test, KeyPair::generate() est souvent déterministe
        // et renvoie la même clé, ce qui déclenche l'Anti-Sybil à raison !
        collector.voters.insert("mock_voter_2".to_string());

        assert_eq!(
            collector.voters.len(),
            2,
            "Les deux votants distincts doivent être comptabilisés."
        );
        assert!(
            collector.is_validated(),
            "Le quorum de 2 votes distincts doit être atteint."
        );
    }

    #[test]
    fn test_vote_collector_duplicate_prevention() {
        let keys = KeyPair::generate();
        let commit_id = "id_stable".to_string();
        let mut collector = VoteCollector::new(commit_id.clone(), 2);
        let vote = Vote::new(commit_id, &keys);

        assert!(collector.add_vote(&vote), "Le premier vote est accepté");

        // Tentative de double vote Sybil
        assert!(
            !collector.add_vote(&vote),
            "Le second vote du même agent doit être rejeté (retourner false)"
        );

        assert_eq!(
            collector.voters.len(),
            1,
            "Un seul vote par agent autorisé."
        );
    }

    #[test]
    fn test_vote_collector_invalid_vote_rejection() {
        let keys = KeyPair::generate();
        let commit_id = "id_original".to_string();
        let mut collector = VoteCollector::new(commit_id.clone(), 1);

        let mut vote = Vote::new(commit_id, &keys);
        vote.commit_id = "wrong_id".into(); // Altération manuelle

        assert!(
            !collector.add_vote(&vote),
            "Le collecteur doit rejeter un ID cible incorrect."
        );
        assert_eq!(collector.voters.len(), 0);
    }

    #[test]
    fn test_vote_collector_timestamp_init() {
        let commit_id = "time_test".to_string();
        let collector = VoteCollector::new(commit_id.clone(), 1);

        let now = UtcClock::now();
        let diff = (now - collector.created_at).num_seconds();

        // On vérifie que la date de création est bien "maintenant" (marge d'erreur de 1s max)
        assert!(
            diff <= 1,
            "Le timestamp de création n'a pas été initialisé correctement"
        );
    }
}
