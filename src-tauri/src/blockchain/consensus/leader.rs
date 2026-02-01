// src-tauri/src/blockchain/consensus/leader.rs
//! Logique de sélection du Leader pour le cycle de consensus Arcadia.

use crate::blockchain::vpn::Peer;

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

    /// Sélectionne un leader de manière déterministe à partir de la liste des pairs.
    /// Utilise un round-robin basé sur l'index du bloc via un tri alphabétique des clés.
    pub fn select_leader(&mut self, peers: &[Peer], block_index: u64) -> Option<String> {
        if peers.is_empty() {
            return None;
        }

        // Tri déterministe des pairs par clé publique pour la cohérence inter-nœuds.
        let mut sorted_peers: Vec<String> = peers.iter().map(|p| p.public_key.clone()).collect();
        sorted_peers.sort();

        // Sélection par modulo (Round-Robin)
        let index = (block_index % sorted_peers.len() as u64) as usize;
        let leader = sorted_peers.get(index).cloned();

        self.current_leader_key = leader.clone();
        leader
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

// --- TESTS UNITAIRES ---

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
    fn test_deterministic_leader_selection() {
        let mut election = LeaderElection::new();
        let peers = vec![mock_peer("key_C"), mock_peer("key_A"), mock_peer("key_B")];

        // Pour l'index 0, après tri (A, B, C), le leader doit être "key_A"
        let leader_0 = election.select_leader(&peers, 0);
        assert_eq!(leader_0, Some("key_A".into()));
        assert!(election.is_leader("key_A"));

        // Pour l'index 1, le leader doit être "key_B"
        let leader_1 = election.select_leader(&peers, 1);
        assert_eq!(leader_1, Some("key_B".into()));

        // Pour l'index 3 (modulo), on revient à "key_A"
        let leader_3 = election.select_leader(&peers, 3);
        assert_eq!(leader_3, Some("key_A".into()));
    }

    #[test]
    fn test_default_impl() {
        let election = LeaderElection::default();
        assert!(election.current_leader_key.is_none());
    }

    #[test]
    fn test_empty_peers() {
        let mut election = LeaderElection::new();
        assert_eq!(election.select_leader(&vec![], 0), None);
    }
}
