// src-tauri/src/commands/blockchain_commands.rs
//! Commandes Tauri liées à la Blockchain et au VPN mesh.

use serde::{Deserialize, Serialize};

use crate::blockchain::{
    error::BlockchainError,
    fabric_state,   // Helper pour Fabric
    innernet_state, // Helper pour Innernet
    vpn::innernet_client::{NetworkStatus as VpnStatus, Peer as VpnPeer},
};

// --- DTOs pour le Frontend ---

#[derive(Debug, Serialize, Deserialize)]
pub struct TransactionResult {
    pub success: bool,
    pub message: String,
    #[serde(default)]
    pub payload: Option<serde_json::Value>,
}

// --- COMMANDES FABRIC ---

/// Vérifie l'état du client Fabric (remplace l'ancien ping).
#[tauri::command]
pub async fn fabric_ping(app: tauri::AppHandle) -> Result<String, BlockchainError> {
    // On vérifie juste qu'on peut accéder au state
    let _client = {
        let state = fabric_state(&app);
        let guard = state
            .lock()
            .map_err(|_| BlockchainError::Unknown("Fabric Mutex poisoned".into()))?;
        guard.clone()
    };

    Ok("Fabric Client Ready (v2)".to_string())
}

#[tauri::command]
pub async fn fabric_submit_transaction(
    app: tauri::AppHandle,
    chaincode: String,
    function: String,
    args: Vec<String>,
) -> Result<TransactionResult, BlockchainError> {
    // 1. Récupération thread-safe du client
    let client = {
        let state = fabric_state(&app);
        let guard = state
            .lock()
            .map_err(|_| BlockchainError::Unknown("Fabric Mutex poisoned".into()))?;
        guard.clone()
    };

    // 2. Appel asynchrone (non-bloquant pour l'UI)
    let tx_id = client
        .submit_transaction(&chaincode, &function, args)
        .await?;

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
) -> Result<TransactionResult, BlockchainError> {
    let client = {
        let state = fabric_state(&app);
        let guard = state
            .lock()
            .map_err(|_| BlockchainError::Unknown("Fabric Mutex poisoned".into()))?;
        guard.clone()
    };

    // Conversion des args String -> Vec<u8>
    let byte_args: Vec<Vec<u8>> = args.into_iter().map(|s| s.into_bytes()).collect();

    let result_bytes = client.query_transaction(&function, byte_args).await?;
    let result_str = String::from_utf8_lossy(&result_bytes).to_string();

    Ok(TransactionResult {
        success: true,
        message: "Query successful".to_string(),
        payload: Some(serde_json::json!({ "data": result_str, "chaincode": chaincode })),
    })
}

#[tauri::command]
pub async fn fabric_get_history(
    app: tauri::AppHandle,
    key: String,
) -> Result<TransactionResult, BlockchainError> {
    let client = {
        let state = fabric_state(&app);
        let guard = state
            .lock()
            .map_err(|_| BlockchainError::Unknown("Fabric Mutex poisoned".into()))?;
        guard.clone()
    };

    let args = vec![key.into_bytes()];
    let result = client.query_transaction("GetHistory", args).await?;

    Ok(TransactionResult {
        success: true,
        message: "History retrieved".to_string(),
        payload: Some(serde_json::json!({ "history": String::from_utf8_lossy(&result) })),
    })
}

// --- COMMANDES VPN (INNERNET) ---

#[tauri::command]
pub async fn vpn_network_status(app: tauri::AppHandle) -> Result<VpnStatus, BlockchainError> {
    let client = {
        let state = innernet_state(&app);
        let guard = state
            .lock()
            .map_err(|_| BlockchainError::Unknown("VPN Mutex poisoned".into()))?;
        guard.clone()
    };
    client.get_status().await.map_err(BlockchainError::from)
}

#[tauri::command]
pub async fn vpn_connect(app: tauri::AppHandle) -> Result<(), BlockchainError> {
    let client = {
        let state = innernet_state(&app);
        let guard = state
            .lock()
            .map_err(|_| BlockchainError::Unknown("VPN Mutex poisoned".into()))?;
        guard.clone()
    };
    client.connect().await.map_err(BlockchainError::from)
}

#[tauri::command]
pub async fn vpn_disconnect(app: tauri::AppHandle) -> Result<(), BlockchainError> {
    let client = {
        let state = innernet_state(&app);
        let guard = state
            .lock()
            .map_err(|_| BlockchainError::Unknown("VPN Mutex poisoned".into()))?;
        guard.clone()
    };
    client.disconnect().await.map_err(BlockchainError::from)
}

#[tauri::command]
pub async fn vpn_list_peers(app: tauri::AppHandle) -> Result<Vec<VpnPeer>, BlockchainError> {
    let client = {
        let state = innernet_state(&app);
        let guard = state
            .lock()
            .map_err(|_| BlockchainError::Unknown("VPN Mutex poisoned".into()))?;
        guard.clone()
    };
    client.list_peers().await.map_err(BlockchainError::from)
}

#[tauri::command]
pub async fn vpn_add_peer(
    app: tauri::AppHandle,
    invitation_code: String,
) -> Result<String, BlockchainError> {
    let client = {
        let state = innernet_state(&app);
        let guard = state
            .lock()
            .map_err(|_| BlockchainError::Unknown("VPN Mutex poisoned".into()))?;
        guard.clone()
    };
    client
        .add_peer(&invitation_code)
        .await
        .map_err(BlockchainError::from)
}

#[tauri::command]
pub async fn vpn_ping_peer(
    app: tauri::AppHandle,
    peer_ip: String,
) -> Result<bool, BlockchainError> {
    let client = {
        let state = innernet_state(&app);
        let guard = state
            .lock()
            .map_err(|_| BlockchainError::Unknown("VPN Mutex poisoned".into()))?;
        guard.clone()
    };
    client
        .ping_peer(&peer_ip)
        .await
        .map_err(BlockchainError::from)
}

#[tauri::command]
pub fn vpn_check_installation() -> bool {
    crate::blockchain::InnernetClient::check_installation().is_ok()
}
