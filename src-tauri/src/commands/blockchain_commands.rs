// src-tauri/src/commands/blockchain_commands.rs
//! Commandes Tauri pour le Marketplace Mentis : L'interface entre l'UI et le Ledger.

use crate::blockchain::{
    crypto::signing::KeyPair,
    ensure_blockchain_client,
    p2p::{MentisBehavior, MentisNetMessage},
    storage::chain::Ledger,
    storage::commit::{MentisCommit, Mutation},
    BlockchainState, NetworkConfig,
};
use crate::utils::prelude::*;
use libp2p::{gossipsub, Swarm};
use tauri::State;

/// 🚀 Initialise le nœud souverain de l'Agent.
#[tauri::command]
pub async fn mentis_init_node(
    state: State<'_, SharedRef<AsyncMutex<BlockchainState>>>,
    config: NetworkConfig,
) -> RaiseResult<()> {
    ensure_blockchain_client(state.inner().clone(), config).await?;
    user_success!("INF_MENTIS_NODE_READY");
    Ok(())
}

/// 🛒 Signe et diffuse une nouvelle connaissance sur le réseau Mentis.
#[tauri::command]
pub async fn mentis_broadcast_mutation(
    mutation: Mutation,
    swarm_state: State<'_, AsyncMutex<Swarm<MentisBehavior>>>,
    ledger_state: State<'_, SyncMutex<Ledger>>,
) -> RaiseResult<String> {
    // 1. Préparation atomique du bloc Mentis
    let (commit_id, encoded_msg) = {
        // 🎯 FIX : Utilisation d'un match pour utiliser raise_error! et éviter les variants AppError inconnus
        let mut ledger = match ledger_state.lock() {
            Ok(guard) => guard,
            Err(_) => raise_error!("ERR_LEDGER_LOCK", error = "Ledger lock poisoned"),
        };

        // On génère les clés de l'agent pour signer l'acte de vente
        let keys = KeyPair::generate();

        // 🎯 FIX : Utilisation de la signature à 3 arguments validée dans commit.rs
        let commit = MentisCommit::new(vec![mutation], ledger.last_commit_hash.clone(), &keys);

        let current_id = commit.id.clone();

        // 2. Sérialisation via la façade sémantique pour le réseau P2P
        let msg = MentisNetMessage::AnnounceCommit(commit.clone());
        let encoded = json::serialize_to_bytes(&msg)?;

        // 3. Archivage local immédiat
        ledger.append_commit(commit)?;
        (current_id, encoded)
    };

    // 4. Diffusion P2P via Gossipsub
    let mut swarm = swarm_state.lock().await;
    let topic = gossipsub::IdentTopic::new("mentis_market");

    match swarm.behaviour_mut().gossipsub.publish(topic, encoded_msg) {
        Ok(_) => {
            user_info!(
                "INF_MENTIS_BROADCAST",
                json::json_value!({ "commit_id": commit_id })
            );
            Ok(commit_id)
        }
        // 🎯 FIX : Utilisation de la macro raise_error! standard du projet
        Err(e) => raise_error!("ERR_P2P_PUBLISH", error = e.to_string()),
    }
}

/// 📊 Récupère l'état actuel du Ledger Mentis pour l'affichage UI.
#[tauri::command]
pub fn mentis_get_ledger_info(ledger_state: State<'_, SyncMutex<Ledger>>) -> JsonValue {
    match ledger_state.lock() {
        Ok(ledger) => {
            json::json_value!({
                "blocks_count": ledger.len(),
                "head": ledger.last_commit_hash,
                "is_active": !ledger.is_empty(),
                "status": "synchronized"
            })
        }
        // En cas de lock empoisonné, on renvoie une structure JSON d'erreur propre pour l'UI
        Err(_) => json::json_value!({ "error": "LOCK_POISONED", "status": "error" }),
    }
}

// =========================================================================
// TESTS UNITAIRES (Audit des Commandes)
// =========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blockchain::storage::commit::MutationOp;

    #[test]
    fn test_command_dto_parsing_robustness() {
        let raw_json = r#"{
            "@id": "urn:mentis:test",
            "operation": "Create",
            "payload": {"val": 42}
        }"#;

        let mutation: Mutation =
            json::deserialize_from_str(raw_json).expect("Désérialisation DTO échouée");
        assert_eq!(mutation.element_id, "urn:mentis:test");
        assert_eq!(mutation.operation, MutationOp::Create);
    }
}
