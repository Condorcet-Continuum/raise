// src-tauri/src/blockchain/client.rs
//! Client de communication pour le réseau souverain Mentis.

use crate::utils::prelude::*;

/// Configuration du nœud Mentis.
#[derive(Debug, Clone, Serializable, Deserializable)]
pub struct NetworkConfig {
    pub node_name: String,
    pub bootnodes: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct BlockchainClient {
    config: NetworkConfig,
    // 🎯 Ajout du canal de transmission vers le Swarm (Network Service)
    // Note: Typé String pour relayer le commit_id, remplaçable par MentisNetMessage
    network_tx: Option<AsyncChannel::Sender<String>>,
}

impl BlockchainClient {
    /// Crée un nouveau client sans connexion réseau par défaut.
    pub fn new(config: NetworkConfig) -> Self {
        Self {
            config,
            network_tx: None,
        }
    }

    /// Injecte le canal de communication réseau (Dependency Injection).
    pub fn with_network_channel(mut self, tx: AsyncChannel::Sender<String>) -> Self {
        self.network_tx = Some(tx);
        self
    }

    /// Charge la configuration depuis le système de fichiers (YAML).
    pub async fn load_from_path<P: AsRef<fs::Path>>(path: P) -> RaiseResult<Self> {
        let p = path.as_ref();

        // 1. Lecture asynchrone via façade fs
        let content = match fs::read_to_string_async(p).await {
            Ok(c) => c,
            Err(e) => {
                raise_error!(
                    "ERR_BLOCKCHAIN_CONFIG_READ",
                    error = e.to_string(),
                    context = json_value!({ "path": p.to_string_lossy() })
                );
            }
        };

        // 2. Désérialisation YAML sécurisée
        let config: NetworkConfig = match json::deserialize_from_yaml(&content) {
            Ok(cfg) => cfg,
            Err(e) => {
                raise_error!(
                    "ERR_BLOCKCHAIN_CONFIG_PARSE",
                    error = e.to_string(),
                    context = json_value!({ "path": p.to_string_lossy() })
                );
            }
        };

        Ok(Self::new(config))
    }

    /// Diffuse une annonce de bloc sur le réseau GossipSub.
    pub async fn broadcast_announcement(&self, commit_id: &str) -> RaiseResult<()> {
        user_info!("INF_BLOCKCHAIN_BROADCAST", json_value!({ "id": commit_id }));

        if let Some(tx) = &self.network_tx {
            // Envoi asynchrone dans le canal MPSC vers la boucle P2P
            if let Err(e) = tx.send(commit_id.to_string()).await {
                raise_error!(
                    "ERR_BLOCKCHAIN_BROADCAST_FAILED",
                    error = e.to_string(),
                    context = json_value!({ "commit_id": commit_id })
                );
            }
        } else {
            user_warn!(
                "WARN_BLOCKCHAIN_NO_NETWORK",
                "Diffusion ignorée : Aucun canal réseau configuré pour ce client."
            );
        }

        Ok(())
    }

    pub fn config(&self) -> &NetworkConfig {
        &self.config
    }
}

// =========================================================================
// TESTS DE CONFORMITÉ (RUST FIRST)
// =========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[async_test]
    async fn test_blockchain_client_initialization() {
        let config = NetworkConfig {
            node_name: "mentis-core-01".into(),
            bootnodes: vec!["/ip4/127.0.0.1/tcp/4001".into()],
        };

        let client = BlockchainClient::new(config);
        assert_eq!(client.config().node_name, "mentis-core-01");
    }

    #[async_test]
    async fn test_broadcast_interface_robustness() {
        let config = NetworkConfig {
            node_name: "test".into(),
            bootnodes: vec![],
        };
        let client = BlockchainClient::new(config);

        // Vérifie que l'interface ne bloque pas et gère gracieusement l'absence de canal
        let res = client.broadcast_announcement("commit_test_001").await;
        assert!(
            res.is_ok(),
            "Le broadcast sans canal doit réussir silencieusement (avec un log WARN)"
        );
    }

    #[async_test]
    async fn test_broadcast_with_injected_channel() {
        let config = NetworkConfig {
            node_name: "mentis-connected".into(),
            bootnodes: vec![],
        };

        // 1. Initialisation du mock du réseau (Canal MPSC)
        let (tx, mut rx) = AsyncChannel::channel::<String>(10);

        // 2. Création du client avec injection du canal
        let client = BlockchainClient::new(config).with_network_channel(tx);

        // 3. Exécution de la diffusion
        let test_commit_id = "commit_alpha_999";
        let result = client.broadcast_announcement(test_commit_id).await;

        // 4. Assertions sur le retour de la fonction
        assert!(
            result.is_ok(),
            "La fonction de broadcast doit retourner Ok(())"
        );

        // 5. Assertions sur l'effet de bord (vérification que le Swarm recevrait le message)
        let received_message = rx.recv().await;
        assert!(
            received_message.is_some(),
            "Le récepteur du canal aurait dû recevoir un message"
        );
        assert_eq!(
            received_message.unwrap(),
            test_commit_id,
            "Le message reçu par le mock du Swarm doit correspondre au commit_id diffusé"
        );
    }
}
