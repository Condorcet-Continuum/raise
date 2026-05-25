// FICHIER : crates/raise-desktop/src/commands/blockchain_commands.rs

use raise_core::utils::prelude::*;

// 🎯 Tout provient désormais de raise_core
use raise_core::blockchain::{
    crypto::signing::KeyPair,
    ensure_blockchain_client,
    p2p::{MentisBehavior, MentisNetMessage},
    storage::chain::Ledger,
    storage::commit::{MentisCommit, Mutation},
    BlockchainState, NetworkConfig,
};

use libp2p::{gossipsub, Swarm};
use tauri::{command, State};

#[command]
pub async fn mentis_init_node(
    state: State<'_, SharedRef<AsyncMutex<BlockchainState>>>,
    config: NetworkConfig,
) -> RaiseResult<()> {
    ensure_blockchain_client(state.inner().clone(), config).await?;
    user_success!("INF_MENTIS_NODE_READY");
    Ok(())
}

#[command]
pub async fn mentis_broadcast_mutation(
    mutation: Mutation,
    swarm_state: State<'_, AsyncMutex<Swarm<MentisBehavior>>>,
    ledger_state: State<'_, SyncMutex<Ledger>>,
) -> RaiseResult<String> {
    let (commit_id, encoded_msg) = {
        let mut ledger = match ledger_state.lock() {
            Ok(guard) => guard,
            Err(_) => raise_error!("ERR_LEDGER_LOCK", error = "Ledger lock poisoned"),
        };

        let keys = KeyPair::generate();
        let commit = MentisCommit::new(vec![mutation], ledger.last_commit_hash.clone(), &keys);
        let current_id = commit.id.clone();

        let msg = MentisNetMessage::AnnounceCommit(commit.clone());
        let encoded = raise_core::utils::prelude::json::serialize_to_bytes(&msg)?;

        ledger.append_commit(commit)?;
        (current_id, encoded)
    };

    let mut swarm = swarm_state.lock().await;
    let topic = gossipsub::IdentTopic::new("mentis_market");

    match swarm.behaviour_mut().gossipsub.publish(topic, encoded_msg) {
        Ok(_) => {
            user_info!(
                "INF_MENTIS_BROADCAST",
                json_value!({ "commit_id": commit_id })
            );
            Ok(commit_id)
        }
        Err(e) => raise_error!("ERR_P2P_PUBLISH", error = e.to_string()),
    }
}

#[command]
pub fn mentis_get_ledger_info(ledger_state: State<'_, SyncMutex<Ledger>>) -> JsonValue {
    match ledger_state.lock() {
        Ok(ledger) => {
            json_value!({
                "blocks_count": ledger.len(),
                "head": ledger.last_commit_hash,
                "is_active": !ledger.is_empty(),
                "status": "synchronized"
            })
        }
        Err(_) => json_value!({ "error": "LOCK_POISONED", "status": "error" }),
    }
}
