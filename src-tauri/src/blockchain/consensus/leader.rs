// src-tauri/src/blockchain/consensus/leader.rs
//! Logique de sélection du Leader pour le cycle de consensus Mentis.

use crate::blockchain::crypto::hashing::calculate_hash;
use crate::blockchain::vpn::Peer;
use crate::utils::prelude::*;

/// Gère l'élection du leader basé sur la liste des pairs du mesh.
pub struct LeaderElection {
    pub current_leader_key: Option<String>,
}

impl LeaderElection {
    /// Crée une nouvelle instance de gestion d'élection.
    pub fn new() -> Self {
        Self {
            current_leader_key: None,
        }
    }

    /// Sélectionne un leader de manière déterministe et résiliente au "Churn" (déconnexions).
    /// Utilise l'algorithme de Rendezvous Hashing (Highest Random Weight).
    pub fn select_leader(&mut self, peers: &[Peer], block_index: u64) -> Option<String> {
        if peers.is_empty() {
            self.current_leader_key = None;
            return None;
        }

        let mut highest_score = String::new();
        let mut selected_leader = None;

        // 🎯 FIX ANTI-FORK : Calcul d'un score cryptographique unique pour chaque pair
        for peer in peers {
            let payload = json_value!({
                "block_index": block_index,
                "pubkey": peer.public_key
            });

            // On génère le hash (le score aléatoire mais déterministe)
            let score = calculate_hash(&payload);

            // Le pair avec le score le plus élevé gagne l'élection pour ce bloc précis
            if selected_leader.is_none() || score > highest_score {
                highest_score = score;
                selected_leader = Some(peer.public_key.clone());
            }
        }

        self.current_leader_key = selected_leader.clone();
        selected_leader
    }

    /// Vérifie si une clé donnée est celle du leader actuel.
    pub fn is_leader(&self, public_key: &str) -> bool {
        self.current_leader_key.as_deref() == Some(public_key)
    }
}

// Implémentation de Default demandée par Clippy pour les constructeurs sans arguments.
impl Default for LeaderElection {
    fn default() -> Self {
        Self::new()
    }
}

// =========================================================================
// TESTS UNITAIRES (Audit de Résilience P2P)
// =========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn mock_peer(key: &str) -> Peer {
        Peer {
            name: format!("node_{}", key).into(),
            public_key: key.into(),
            ip: "10.42.0.1".into(),
            endpoint: None,
            last_handshake: None,
            transfer_rx: 0,
            transfer_tx: 0,
        }
    }

    #[test]
    fn test_rendezvous_hashing_stability() {
        let mut election = LeaderElection::new();
        let peer_a = mock_peer("pubkey_A");
        let peer_b = mock_peer("pubkey_B");
        let peer_c = mock_peer("pubkey_C");

        let peers_full = vec![peer_a.clone(), peer_b.clone(), peer_c.clone()];

        // On détermine le leader pour le bloc 100
        let leader_full = election.select_leader(&peers_full, 100).unwrap();

        // 🎯 FIX DU TEST : On trouve un nœud qui N'EST PAS le leader pour simuler sa déconnexion
        let loser_key = peers_full
            .iter()
            .find(|p| p.public_key != leader_full)
            .unwrap()
            .public_key
            .clone();

        // On retire ce perdant du réseau
        let peers_reduced: Vec<Peer> = peers_full
            .into_iter()
            .filter(|p| p.public_key != loser_key)
            .collect();

        let leader_reduced = election.select_leader(&peers_reduced, 100).unwrap();

        // Le leader DOIT rester le même.
        assert_eq!(
            leader_full, leader_reduced,
            "La déconnexion d'un pair perdant ne doit pas altérer le leader du bloc en cours"
        );
    }

    #[test]
    fn test_leader_election_determinism() {
        let mut election1 = LeaderElection::new();
        let mut election2 = LeaderElection::new();

        let peers = vec![mock_peer("key_A"), mock_peer("key_B"), mock_peer("key_C")];

        // Deux nœuds différents sur le réseau calculant l'élection doivent converger
        let leader1 = election1.select_leader(&peers, 42);
        let leader2 = election2.select_leader(&peers, 42);

        assert_eq!(leader1, leader2, "Le consensus n'est pas déterministe");
    }

    #[test]
    fn test_default_impl_and_empty_peers() {
        let mut election = LeaderElection::default();
        assert!(election.current_leader_key.is_none());
        assert_eq!(election.select_leader(&vec![], 10), None);
    }
}
