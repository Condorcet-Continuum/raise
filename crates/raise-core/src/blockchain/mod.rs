// src-tauri/src/blockchain/mod.rs
//! Module racine Mentis : Orchestration du réseau de connaissance.

use crate::utils::prelude::*;

// --- ARBORESCENCE ---
pub mod bridge; // Adaptateur JsonDB
pub mod client; // Client P2P Principal
pub mod consensus; // Quorum & Votes
pub mod crypto; // Hashing & Signatures
pub mod p2p; // Transport (p2p)
pub mod storage; // Ledger & Commits
pub mod sync; // Synchronisation Delta
pub mod vpn; // Maillage privé (Innernet)

// --- CONTRAT DE VALEUR ---
#[async_interface]
pub trait ValueGateway: Send + Sync {
    async fn verify_payment(&self, commit_id: &str, buyer: &str) -> RaiseResult<bool>;
    async fn trigger_payout(&self, commit_id: &str, seller: &str) -> RaiseResult<()>;
}

// --- RÉEXPORTATIONS STRATÉGIQUES ---
pub use client::{BlockchainClient, NetworkConfig};
pub use consensus::ConsensusEngine as MentisConsensus;
pub use storage::chain::Ledger;
pub use storage::commit::{MentisCommit, Mutation, MutationOp};

/// État global injecté dans Tauri.
#[derive(Debug, Clone, Default)]
pub struct BlockchainState {
    pub client: Option<SharedRef<AsyncMutex<BlockchainClient>>>,
}

/// Initialise le client Mentis de manière unique (Singleton).
pub async fn ensure_blockchain_client(
    state: SharedRef<AsyncMutex<BlockchainState>>,
    config: NetworkConfig,
) -> RaiseResult<SharedRef<AsyncMutex<BlockchainClient>>> {
    let mut guard = state.lock().await;

    // Si le client existe déjà, on retourne son clone (pointe vers la même instance)
    if let Some(ref client) = guard.client {
        return Ok(client.clone());
    }

    // Sinon, création et stockage
    let client = BlockchainClient::new(config);
    let shared = SharedRef::new(AsyncMutex::new(client));

    guard.client = Some(shared.clone());

    user_success!("INF_MENTIS_READY");
    Ok(shared)
}

// =========================================================================
// TESTS DE CONFORMITÉ (Audit Global Mentis)
// =========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// Test 1 : Vérification de la persistance du Singleton.
    #[async_test]
    async fn test_mentis_singleton_persistence() {
        let state = SharedRef::new(AsyncMutex::new(BlockchainState::default()));
        let config = NetworkConfig {
            node_name: "node-test".into(),
            bootnodes: vec![],
        };

        let c1 = ensure_blockchain_client(state.clone(), config.clone())
            .await
            .unwrap();
        let c2 = ensure_blockchain_client(state, config).await.unwrap();

        // 🎯 FIX : Utilisation de ptr_eq pour comparer les adresses sur le TAS (Heap)
        // et non les adresses des variables sur la PILE (Stack).
        assert!(
            SharedRef::ptr_eq(&c1, &c2),
            "Le client BlockchainClient doit être physiquement le même (Singleton)."
        );
    }

    /// Test 2 : Vérification de l'interface de consensus.
    #[test]
    fn test_consensus_alias_integrity() {
        let engine = MentisConsensus::new(5);
        assert_eq!(engine.default_quorum, 5);
    }
}
