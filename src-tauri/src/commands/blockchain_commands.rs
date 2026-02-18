// src-tauri/src/commands/blockchain_commands.rs
//! Commandes Tauri liées à la Blockchain (Fabric, VPN Mesh et Arcadia P2P).

use crate::blockchain::{
    fabric_state, innernet_state,
    vpn::innernet_client::{NetworkStatus as VpnStatus, Peer as VpnPeer},
};
use crate::utils::{
    data::{self, Value},
    prelude::*,
};
use std::sync::Mutex;
use tauri::State;
use tokio::sync::Mutex as AsyncMutex;

// --- IMPORTS ARCADIA P2P ---
use crate::blockchain::p2p::behavior::ArcadiaBehavior;
use crate::blockchain::p2p::protocol::ArcadiaNetMessage;
use crate::blockchain::storage::chain::Ledger;
use crate::blockchain::storage::commit::{ArcadiaCommit, Mutation};
use crate::blockchain::sync::engine::SyncEngine;
use crate::blockchain::sync::state::SyncStatus;
use libp2p::{gossipsub, Swarm};

// --- DTOs pour le Frontend ---

#[derive(Debug, Serialize, Deserialize)]
pub struct TransactionResult {
    pub success: bool,
    pub message: String,
    #[serde(default)]
    pub payload: Option<Value>,
}

// --- COMMANDES ARCADIA (P2P SOUVERAIN) ---

/// Diffuse une mutation sur le réseau Arcadia.
#[tauri::command]
pub async fn arcadia_broadcast_mutation(
    mutation: Mutation,
    swarm_state: State<'_, AsyncMutex<Swarm<ArcadiaBehavior>>>,
    ledger_state: State<'_, Mutex<Ledger>>,
) -> Result<String> {
    // 1. On extrait les données du Ledger dans un bloc limité pour relâcher le Mutex immédiatement
    let (commit, encoded_msg) = {
        let mut ledger = ledger_state.lock().map_err(|_| "Ledger Mutex poisoned")?;

        let keys = crate::blockchain::crypto::signing::KeyPair::generate();
        let parent_hash = ledger.last_commit_hash.clone();

        let mut commit = ArcadiaCommit {
            id: String::new(),
            parent_hash,
            author: keys.public_key_hex(),
            timestamp: chrono::Utc::now(),
            mutations: vec![mutation],
            merkle_root: String::new(),
            signature: vec![],
        };

        let hash = commit.compute_content_hash();
        commit.id = hash.clone();
        commit.signature = keys.sign(&hash);

        // On met à jour le ledger local
        ledger
            .append_commit(commit.clone())
            .map_err(|e| format!("Erreur Ledger: {}", e))?;

        let msg = ArcadiaNetMessage::AnnounceCommit(commit.clone());
        let encoded = data::to_vec(&msg).map_err(|e| e.to_string())?;

        (commit, encoded) // Le verrou 'ledger' est relâché ici à la sortie du bloc
    };

    // 2. Maintenant on peut faire le .await du swarm sans tenir le verrou du ledger
    let mut swarm = swarm_state.lock().await;
    let topic = gossipsub::IdentTopic::new("arcadia_commits");

    swarm
        .behaviour_mut()
        .gossipsub
        .publish(topic, encoded_msg)
        .map_err(|e| format!("Erreur de diffusion P2P: {:?}", e))?;

    Ok(commit.id)
}
/// Récupère l'état actuel de la synchronisation Arcadia.
#[tauri::command]
pub fn arcadia_get_sync_status(sync_state: State<'_, Mutex<SyncEngine>>) -> SyncStatus {
    let engine = sync_state.lock().unwrap();
    engine.status.clone()
}

/// Récupère les derniers commits du registre local.
#[tauri::command]
pub fn arcadia_get_ledger_info(ledger_state: State<'_, Mutex<Ledger>>) -> Value {
    let ledger = ledger_state.lock().unwrap();
    data::json!({
        "height": ledger.len(),
        "last_hash": ledger.last_commit_hash,
        "is_empty": ledger.is_empty()
    })
}

// --- COMMANDES FABRIC ---

#[tauri::command]
pub async fn fabric_ping(app: tauri::AppHandle) -> Result<String> {
    let state = fabric_state(&app);
    let _guard = state
        .lock()
        .map_err(|_| AppError::from("Fabric Mutex poisoned"))?;

    Ok("Fabric Client Ready (v2)".to_string())
}

#[tauri::command]
pub async fn fabric_submit_transaction(
    app: tauri::AppHandle,
    chaincode: String,
    function: String,
    args: Vec<String>,
) -> Result<TransactionResult> {
    let client = {
        let state = fabric_state(&app);
        let guard = state
            .lock()
            .map_err(|_| AppError::from("Fabric Mutex poisoned"))?;
        guard.clone()
    };

    let tx_id = client
        .submit_transaction(&chaincode, &function, args)
        .await
        .map_err(|e| AppError::from(e.to_string()))?;

    Ok(TransactionResult {
        success: true,
        message: format!("Transaction submitted: {}", tx_id),
        payload: None,
    })
}

#[tauri::command]
pub async fn fabric_query_transaction(
    app: tauri::AppHandle,
    chaincode: String,
    function: String,
    args: Vec<String>,
) -> Result<TransactionResult> {
    let client = {
        let state = fabric_state(&app);
        let guard = state
            .lock()
            .map_err(|_| AppError::from("Fabric Mutex poisoned"))?;
        guard.clone()
    };

    let byte_args: Vec<Vec<u8>> = args.into_iter().map(|s| s.into_bytes()).collect();
    let result_bytes = client
        .query_transaction(&function, byte_args)
        .await
        .map_err(|e| AppError::from(e.to_string()))?;

    let result_str = String::from_utf8_lossy(&result_bytes).to_string();

    Ok(TransactionResult {
        success: true,
        message: "Query successful".to_string(),
        payload: Some(data::json!({ "data": result_str, "chaincode": chaincode })),
    })
}

#[tauri::command]
pub async fn fabric_get_history(app: tauri::AppHandle, key: String) -> Result<TransactionResult> {
    let client = {
        let state = fabric_state(&app);
        let guard = state
            .lock()
            .map_err(|_| AppError::from("Fabric Mutex poisoned"))?;
        guard.clone()
    };

    let args = vec![key.into_bytes()];
    let result = client
        .query_transaction("GetHistory", args)
        .await
        .map_err(|e| AppError::from(e.to_string()))?;

    Ok(TransactionResult {
        success: true,
        message: "History retrieved".to_string(),
        payload: Some(data::json!({ "history": String::from_utf8_lossy(&result) })),
    })
}

// --- COMMANDES VPN (INNERNET) ---

#[tauri::command]
pub async fn vpn_network_status(app: tauri::AppHandle) -> Result<VpnStatus> {
    let client = {
        let state = innernet_state(&app);
        let guard = state
            .lock()
            .map_err(|_| AppError::from("VPN Mutex poisoned"))?;
        guard.clone()
    };
    client
        .get_status()
        .await
        .map_err(|e| AppError::from(e.to_string()))
}

#[tauri::command]
pub async fn vpn_connect(app: tauri::AppHandle) -> Result<()> {
    let client = {
        let state = innernet_state(&app);
        let guard = state
            .lock()
            .map_err(|_| AppError::from("VPN Mutex poisoned"))?;
        guard.clone()
    };
    client
        .connect()
        .await
        .map_err(|e| AppError::from(e.to_string()))
}

#[tauri::command]
pub async fn vpn_disconnect(app: tauri::AppHandle) -> Result<()> {
    let client = {
        let state = innernet_state(&app);
        let guard = state
            .lock()
            .map_err(|_| AppError::from("VPN Mutex poisoned"))?;
        guard.clone()
    };
    client
        .disconnect()
        .await
        .map_err(|e| AppError::from(e.to_string()))
}

#[tauri::command]
pub async fn vpn_list_peers(app: tauri::AppHandle) -> Result<Vec<VpnPeer>> {
    let client = {
        let state = innernet_state(&app);
        let guard = state
            .lock()
            .map_err(|_| AppError::from("VPN Mutex poisoned"))?;
        guard.clone()
    };
    client
        .list_peers()
        .await
        .map_err(|e| AppError::from(e.to_string()))
}

#[tauri::command]
pub async fn vpn_add_peer(app: tauri::AppHandle, invitation_code: String) -> Result<String> {
    let client = {
        let state = innernet_state(&app);
        let guard = state
            .lock()
            .map_err(|_| AppError::from("VPN Mutex poisoned"))?;
        guard.clone()
    };
    client
        .add_peer(&invitation_code)
        .await
        .map_err(|e| AppError::from(e.to_string()))
}

#[tauri::command]
pub async fn vpn_ping_peer(app: tauri::AppHandle, peer_ip: String) -> Result<bool> {
    let client = {
        let state = innernet_state(&app);
        let guard = state
            .lock()
            .map_err(|_| AppError::from("VPN Mutex poisoned"))?;
        guard.clone()
    };
    client
        .ping_peer(&peer_ip)
        .await
        .map_err(|e| AppError::from(e.to_string()))
}

#[tauri::command]
pub fn vpn_check_installation() -> bool {
    crate::blockchain::vpn::innernet_client::InnernetClient::check_installation().is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transaction_result_serialization() {
        let res = TransactionResult {
            success: true,
            message: "Test OK".into(),
            payload: Some(data::json!({"id": 1})),
        };
        let json = data::stringify(&res).unwrap();
        assert!(json.contains("\"success\":true"));
    }
}
