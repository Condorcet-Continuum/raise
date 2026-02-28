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
) -> RaiseResult<String> {
    // 1. Phase Ledger (Synchronisée)
    let (commit_id, encoded_msg) = {
        let mut ledger = match ledger_state.lock() {
            Ok(guard) => guard,
            Err(_) => raise_error!(
                "ERR_LEDGER_MUTEX_POISONED",
                context = json!({
                    "component": "LedgerState",
                    "action": "access_ledger_storage",
                    "hint": "Le Mutex du Ledger est corrompu suite à une panique dans un thread précédent. Un redémarrage du moteur de transaction est nécessaire."
                })
            ),
        };

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

        let current_id = commit.id.clone();

        // Sérialisation AVANT le transfert de propriété au ledger
        let msg = ArcadiaNetMessage::AnnounceCommit(commit.clone());
        let encoded = match data::to_vec(&msg) {
            Ok(v) => v,
            Err(e) => raise_error!(
                "ERR_NET_SERIALIZATION_FAILED",
                error = e,
                context = json!({ "action": "encode_commit_for_p2p", "commit_id": current_id })
            ),
        };

        // Transfert de propriété final au ledger
        if let Err(e) = ledger.append_commit(commit) {
            raise_error!(
                "ERR_LEDGER_APPEND_FAILED",
                error = e,
                context = json!({
                    "action": "persist_local_commit",
                    "commit_id": current_id
                })
            );
        }

        (current_id, encoded)
    };

    // 2. Phase P2P (Asynchrone)
    let mut swarm = swarm_state.lock().await;
    let topic = gossipsub::IdentTopic::new("arcadia_commits");

    if let Err(e) = swarm.behaviour_mut().gossipsub.publish(topic, encoded_msg) {
        raise_error!(
            "ERR_P2P_PUBLISH_FAILED",
            error = e,
            context = json!({
                "action": "broadcast_mutation",
                "topic": "arcadia_commits",
                "commit_id": commit_id
            })
        );
    }

    Ok(commit_id)
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
pub async fn fabric_ping(app: tauri::AppHandle) -> RaiseResult<String> {
    let state = fabric_state(&app);
    let _guard = match state.lock() {
        Ok(guard) => guard,
        Err(_) => {
            raise_error!(
                "ERR_MUTEX_POISONED",
                error = "FABRIC_LOCK_CONTAMINATED",
                context = json!({
                    "action": "acquire_fabric_lock",
                    "resource": "FabricState",
                    "hint": "Un thread a paniqué en tenant ce verrou. L'état interne peut être instable. Redémarrage recommandé."
                })
            );
        }
    };

    Ok("Fabric Client Ready (v2)".to_string())
}

#[tauri::command]
pub async fn fabric_submit_transaction(
    app: tauri::AppHandle,
    chaincode: String,
    function: String,
    args: Vec<String>,
) -> RaiseResult<TransactionResult> {
    let client = {
        let state = fabric_state(&app);

        // 1. Tentative d'acquisition du verrou
        let guard = match state.lock() {
            Ok(g) => g,
            Err(_) => {
                raise_error!(
                    "ERR_FABRIC_MUTEX_POISONED",
                    error = "LOCK_CONTAMINATED",
                    context = json!({
                        "action": "clone_fabric_client",
                        "resource": "FabricState",
                        "hint": "Un thread a paniqué en tenant ce verrou. L'état du client Fabric est incertain."
                    })
                );
            }
        };

        // 2. Le clone est extrait, le guard sera relâché à la sortie de l'accolade
        guard.clone()
    };
    let tx_id = match client.submit_transaction(&chaincode, &function, args).await {
        Ok(id) => id,
        Err(e) => {
            raise_error!(
                "ERR_FABRIC_TRANSACTION_SUBMISSION",
                error = e,
                context = json!({
                    "action": "submit_blockchain_tx",
                    "chaincode": chaincode,
                    "function": function,
                    "hint": "Échec de l'endossement ou de la validation. Vérifiez les logs du peer."
                })
            );
        }
    };

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
) -> RaiseResult<TransactionResult> {
    let client = {
        let state = fabric_state(&app);

        // 1. Acquisition sécurisée avec gestion de l'empoisonnement
        let guard = match state.lock() {
            Ok(g) => g,
            Err(_) => {
                raise_error!(
                    "ERR_FABRIC_MUTEX_POISONED",
                    error = "LOCK_CONTAMINATION",
                    context = json!({
                        "action": "extract_fabric_client",
                        "resource": "FabricState",
                        "hint": "Un thread a paniqué en tenant ce verrou. L'état du client peut être corrompu."
                    })
                );
            }
        };

        // 2. Le clone est renvoyé et le guard est relâché à la fermeture de l'accolade
        guard.clone()
    };

    let byte_args: Vec<Vec<u8>> = args.into_iter().map(|s| s.into_bytes()).collect();
    let result_bytes = match client.query_transaction(&function, byte_args).await {
        Ok(bytes) => bytes,
        Err(e) => raise_error!(
            "ERR_FABRIC_QUERY_FAILED",
            error = e,
            context = json!({
                "action": "query_blockchain_state",
                "function": function,
                "hint": "Échec de la lecture sur le ledger. Vérifiez si la clé existe et si les arguments sont corrects."
            })
        ),
    };

    let result_str = String::from_utf8_lossy(&result_bytes).to_string();

    Ok(TransactionResult {
        success: true,
        message: "Query successful".to_string(),
        payload: Some(data::json!({ "data": result_str, "chaincode": chaincode })),
    })
}

#[tauri::command]
pub async fn fabric_get_history(
    app: tauri::AppHandle,
    key: String,
) -> RaiseResult<TransactionResult> {
    // 1. Extraction sécurisée du client (Mutex déjà blindé !)
    let client = {
        let state = fabric_state(&app);
        let guard = match state.lock() {
            Ok(g) => g,
            Err(_) => raise_error!(
                "ERR_FABRIC_MUTEX_POISONED",
                error = "LOCK_CONTAMINATION",
                context = json!({ "action": "access_fabric_state", "resource": "HistoryQuery" })
            ),
        };
        guard.clone()
    };

    let args = vec![key.clone().into_bytes()];

    // 2. Requête d'historique avec capture de contexte
    let result = match client.query_transaction("GetHistory", args).await {
        Ok(res) => res,
        Err(e) => raise_error!(
            "ERR_FABRIC_HISTORY_FETCH",
            error = e,
            context = json!({
                "action": "fetch_key_history",
                "target_key": key,
                "hint": "Impossible de récupérer l'historique. Vérifiez si la clé existe et si l'utilisateur a les droits de lecture."
            })
        ),
    };

    Ok(TransactionResult {
        success: true,
        message: format!("Historique pour la clé '{}' récupéré", key),
        payload: Some(data::json!({
            "history": String::from_utf8_lossy(&result)
        })),
    })
}

// --- COMMANDES VPN (INNERNET) ---

#[tauri::command]
pub async fn vpn_network_status(app: tauri::AppHandle) -> RaiseResult<VpnStatus> {
    // 1. Extraction sécurisée du client VPN
    let client = {
        let state = innernet_state(&app);

        let guard = match state.lock() {
            Ok(g) => g,
            Err(_) => {
                raise_error!(
                    "ERR_VPN_MUTEX_POISONED",
                    error = "LOCK_CONTAMINATION",
                    context = json!({
                        "action": "access_vpn_state",
                        "resource": "InnernetClient",
                        "hint": "Un thread a paniqué en manipulant l'interface VPN. Redémarrage du service requis."
                    })
                );
            }
        };
        guard.clone()
    };

    // 2. Requête d'état de l'interface réseau
    match client.get_status().await {
        Ok(status) => Ok(status),
        Err(e) => raise_error!(
            "ERR_VPN_STATUS_FETCH",
            error = e,
            context = json!({
                "action": "fetch_innernet_status",
                "interface": "innernet0",
                "hint": "Impossible de lire l'état du VPN. Vérifiez si WireGuard est installé et si l'interface est active."
            })
        ),
    }
}

#[tauri::command]
pub async fn vpn_connect(app: tauri::AppHandle) -> RaiseResult<()> {
    // 1. Extraction sécurisée du client VPN
    let client = {
        let state = innernet_state(&app);
        let guard = match state.lock() {
            Ok(g) => g,
            Err(_) => {
                raise_error!(
                    "ERR_VPN_MUTEX_POISONED",
                    error = "LOCK_CONTAMINATION",
                    context = json!({
                        "action": "establish_vpn_connection",
                        "resource": "InnernetClient"
                    })
                );
            }
        };
        guard.clone()
    };

    // 2. Tentative de connexion avec capture d'erreur réseau
    match client.connect().await {
        Ok(_) => Ok(()),
        Err(e) => raise_error!(
            "ERR_VPN_CONNECTION_FAILED",
            error = e,
            context = json!({
                "action": "up_innernet_interface",
                "interface": "innernet0",
                "hint": "La connexion a échoué. Vérifiez vos clés WireGuard, votre connexion internet ou si une instance innernet tourne déjà."
            })
        ),
    }
}

#[tauri::command]
pub async fn vpn_disconnect(app: tauri::AppHandle) -> RaiseResult<()> {
    // 1. Extraction sécurisée avec gestion de l'empoisonnement
    let client = {
        let state = innernet_state(&app);
        let guard = match state.lock() {
            Ok(g) => g,
            Err(_) => {
                raise_error!(
                    "ERR_VPN_MUTEX_POISONED",
                    error = "LOCK_CONTAMINATION",
                    context = json!({
                        "action": "teardown_vpn_connection",
                        "resource": "InnernetClient"
                    })
                );
            }
        };
        guard.clone()
    };

    // 2. Tentative de déconnexion et nettoyage de l'interface
    match client.disconnect().await {
        Ok(_) => Ok(()),
        Err(e) => raise_error!(
            "ERR_VPN_DISCONNECT_FAILED",
            error = e,
            context = json!({
                "action": "down_innernet_interface",
                "interface": "innernet0",
                "hint": "La déconnexion a échoué. L'interface réseau est peut-être restée dans un état instable."
            })
        ),
    }
}

#[tauri::command]
pub async fn vpn_list_peers(app: tauri::AppHandle) -> RaiseResult<Vec<VpnPeer>> {
    // 1. Extraction sécurisée du client VPN
    let client = {
        let state = innernet_state(&app);
        let guard = match state.lock() {
            Ok(g) => g,
            Err(_) => {
                raise_error!(
                    "ERR_VPN_MUTEX_POISONED",
                    error = "LOCK_CONTAMINATION",
                    context = json!({
                        "action": "list_peers_access",
                        "resource": "InnernetClient"
                    })
                );
            }
        };
        guard.clone()
    };

    // 2. Récupération de la liste des pairs avec contexte
    match client.list_peers().await {
        Ok(peers) => Ok(peers),
        Err(e) => raise_error!(
            "ERR_VPN_LIST_PEERS_FAILED",
            error = e,
            context = json!({
                "action": "query_vpn_topology",
                "interface": "innernet0",
                "hint": "Impossible de récupérer la liste des pairs. Vérifiez si l'interface VPN est active."
            })
        ),
    }
}

#[tauri::command]
pub async fn vpn_add_peer(app: tauri::AppHandle, invitation_code: String) -> RaiseResult<String> {
    // 1. Extraction sécurisée du client VPN
    let client = {
        let state = innernet_state(&app);
        let guard = match state.lock() {
            Ok(g) => g,
            Err(_) => {
                raise_error!(
                    "ERR_VPN_MUTEX_POISONED",
                    error = "LOCK_CONTAMINATION",
                    context = json!({
                        "action": "add_peer_access",
                        "resource": "InnernetClient"
                    })
                );
            }
        };
        guard.clone()
    };

    // 2. Tentative d'ajout avec capture de l'erreur d'invitation
    match client.add_peer(&invitation_code).await {
        Ok(peer_id) => Ok(peer_id),
        Err(e) => raise_error!(
            "ERR_VPN_ADD_PEER_FAILED",
            error = e,
            context = json!({
                "action": "enroll_new_peer",
                "invitation_preview": invitation_code.chars().take(8).collect::<String>() + "...",
                "hint": "L'invitation a échoué. Vérifiez si le code est expiré ou si le serveur d'invitation est accessible."
            })
        ),
    }
}

#[tauri::command]
pub async fn vpn_ping_peer(app: tauri::AppHandle, peer_ip: String) -> RaiseResult<bool> {
    // 1. Extraction sécurisée avec gestion de l'empoisonnement
    let client = {
        let state = innernet_state(&app);
        let guard = match state.lock() {
            Ok(g) => g,
            Err(_) => {
                raise_error!(
                    "ERR_VPN_MUTEX_POISONED",
                    error = "LOCK_CONTAMINATION",
                    context = json!({
                        "action": "ping_peer_access",
                        "resource": "InnernetClient"
                    })
                );
            }
        };
        guard.clone()
    };

    // 2. Tentative de ping avec capture du contexte IP
    match client.ping_peer(&peer_ip).await {
        Ok(reachable) => Ok(reachable),
        Err(e) => raise_error!(
            "ERR_VPN_PING_FAILED",
            error = e,
            context = json!({
                "action": "check_peer_reachability",
                "target_ip": peer_ip,
                "hint": "Le test de connectivité a échoué. Vérifiez si le pair est en ligne et si l'interface VPN est active."
            })
        ),
    }
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
