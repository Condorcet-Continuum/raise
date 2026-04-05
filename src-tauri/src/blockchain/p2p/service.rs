// FICHIER : src-tauri/src/blockchain/p2p/service.rs

use futures::StreamExt;
use libp2p::swarm::SwarmEvent;
use tauri::{AppHandle, Manager};

use crate::blockchain::bridge::ArcadiaBridge; // 🎯 Import du Bridge
use crate::blockchain::consensus::{ConsensusEngine, Vote}; // 🎯 Import du Vote
use crate::blockchain::p2p::behavior::ArcadiaBehavior;
use crate::blockchain::p2p::behavior::ArcadiaBehaviorEvent;
use crate::blockchain::p2p::protocol::ArcadiaNetMessage;
use crate::blockchain::p2p::swarm::create_swarm;
use crate::blockchain::storage::chain::Ledger;
use crate::blockchain::sync::engine::SyncEngine;
use crate::json_db::storage::StorageEngine; // 🎯 Import du Stockage
use crate::utils::prelude::*;
use crate::AppState;

/// Point d'entrée unique pour initialiser et démarrer tout le réseau P2P d'Arcadia.
#[allow(clippy::await_holding_lock)]
pub fn init_arcadia_network(app_handle: AppHandle) {
    // 1. Génération de l'identité locale
    let local_key = libp2p::identity::Keypair::generate_ed25519();
    let local_peer_id = local_key.public().to_peer_id().to_string();

    // 2. Création du Swarm
    let swarm_res = tauri::async_runtime::block_on(async { create_swarm(local_key).await });

    if let Ok(swarm) = swarm_res {
        // 3. Création du canal MPSC
        let (swarm_tx, swarm_rx) = AsyncChannel::channel::<ArcadiaNetMessage>(100);

        // 4. Injection des états dans Tauri
        app_handle.manage(swarm_tx);
        app_handle.manage(AsyncMutex::new(Ledger::new()));
        app_handle.manage(AsyncMutex::new(SyncEngine::new()));

        // 5. Récupération des pairs VPN
        let innernet = crate::blockchain::innernet_state(&app_handle);
        let peers_res = tauri::async_runtime::block_on(async {
            // 🎯 Remplacement de .unwrap() par .await pour l'AsyncMutex !
            innernet.lock().await.list_peers().await
        });

        // 6. Configuration du Consensus et lancement du service
        if let Ok(peers) = peers_res {
            let consensus = ConsensusEngine::new(&peers, 0.5);
            app_handle.manage(AsyncMutex::new(consensus));

            // On lance la boucle en arrière-plan
            spawn_p2p_service(app_handle.clone(), swarm, swarm_rx, local_peer_id);

            println!("✅ [Arcadia] Swarm, Ledger, Consensus et Service P2P initialisés.");
        } else {
            eprintln!("⚠️ [Arcadia] Impossible de récupérer les pairs VPN.");
        }
    } else {
        eprintln!("❌ [Arcadia] Échec du démarrage du réseau P2P.");
    }
}

/// Démarre la boucle réseau P2P en arrière-plan.
pub fn spawn_p2p_service(
    app_handle: AppHandle,
    mut swarm: libp2p::Swarm<ArcadiaBehavior>,
    mut swarm_rx: AsyncChannel::Receiver<ArcadiaNetMessage>,
    local_peer_id: String, // 🎯 NOUVEAU : On passe l'ID du pair local
) {
    tauri::async_runtime::spawn(async move {
        // On récupère les états partagés depuis Tauri
        let consensus_state = app_handle.state::<AsyncMutex<ConsensusEngine>>();
        let storage_state = app_handle.state::<StorageEngine>();
        let app_state = app_handle.state::<SharedRef<AppState>>();

        loop {
            AgentAttention! {
                // 1. ÉCOUTE DU RÉSEAU P2P
                event = swarm.select_next_some() => {
                    if let SwarmEvent::Behaviour(ArcadiaBehaviorEvent::Gossipsub(libp2p::gossipsub::Event::Message { message, .. })) = event {

                        // 🎯 DÉSÉRIALISATION (net_msg est maintenant utilisé)
                        if let Ok(net_msg) = json::deserialize_from_bytes::<ArcadiaNetMessage>(&message.data) {

                            // On verrouille le consensus juste le temps du traitement
                            let mut engine = consensus_state.lock().await; // 🎯 consensus_state est utilisé !

                            match net_msg {
                                ArcadiaNetMessage::AnnounceCommit(commit) => {
                                    if engine.verify_authority(&commit) {
                                        let _ = engine.register_proposal(commit.clone());

                                        // On génère notre vote
                                        let my_vote = Vote {
                                            commit_id: commit.id.clone(),
                                            validator_key: local_peer_id.clone(),
                                            signature: vec![1, 0, 1, 0], // Simulation signature
                                        };

                                        // On publie notre vote sur le réseau
                                        if let Ok(vote_data) = json::serialize_to_bytes(&ArcadiaNetMessage::SubmitVote(my_vote)) {
                                            let topic = libp2p::gossipsub::IdentTopic::new("arcadia-consensus");
                                            let _ = swarm.behaviour_mut().gossipsub.publish(topic, vote_data);
                                        }
                                    }
                                },
                                ArcadiaNetMessage::SubmitVote(vote) => {
                                    if let Ok(Some(final_commit)) = engine.process_vote(vote) {
                                        // Le consensus est atteint ! On l'applique à la base de données via le Bridge
                                        let bridge = ArcadiaBridge::new(storage_state.inner(), app_state.inner());

                                        // Ne pas bloquer la boucle : idéalement ceci devrait être spawn dans une task
                                        let _ = bridge.process_new_commit(&final_commit).await;
                                        engine.finalize_commit(&final_commit.id);
                                    }
                                },
                                _ => {}
                            }
                        }
                    }
                },

                // 2. ÉCOUTE DES COMMANDES INTERNES (Interface UI)
                Some(command) = swarm_rx.recv() => {
                    if let Ok(data) = json::serialize_to_bytes(&command) {
                        let topic = libp2p::gossipsub::IdentTopic::new("arcadia-consensus");
                        let _ = swarm.behaviour_mut().gossipsub.publish(topic, data);
                    }
                }
            }
        }
    });
}

// ============================================================================
// TESTS UNITAIRES
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blockchain::consensus::Vote;
    use crate::blockchain::p2p::protocol::ArcadiaNetMessage;
    use crate::blockchain::storage::commit::ArcadiaCommit;

    #[async_test]
    async fn test_p2p_channel_communication() {
        // Test du canal MPSC utilisé pour la communication Tauri -> Service P2P
        let (tx, mut rx) = AsyncChannel::channel::<ArcadiaNetMessage>(100);

        // Simulation d'une commande envoyée par l'interface UI (ex: arcadia_broadcast_mutation)
        let commit = ArcadiaCommit {
            id: "test_commit_123".to_string(),
            parent_hash: None,
            author: "local_peer".to_string(),
            timestamp: UtcClock::now(),
            mutations: vec![],
            merkle_root: "root_hash".to_string(),
            signature: vec![],
        };

        let msg = ArcadiaNetMessage::AnnounceCommit(commit);

        // Envoi dans le canal (Ne doit pas bloquer)
        assert!(
            tx.send(msg).await.is_ok(),
            "L'envoi dans le canal P2P a échoué"
        );

        // Réception côté "Service P2P" (La boucle interne)
        let received = rx.recv().await;
        assert!(received.is_some(), "Le service P2P n'a rien reçu");

        if let Some(ArcadiaNetMessage::AnnounceCommit(c)) = received {
            assert_eq!(
                c.id, "test_commit_123",
                "L'ID du commit a été altéré dans le canal"
            );
        } else {
            panic!("Mauvais type de message reçu depuis le canal !");
        }
    }

    #[test]
    fn test_gossipsub_payload_parsing() {
        // Test critique : vérifie que le parsing du payload Gossipsub fonctionne
        // exactement comme attendu dans la boucle 'select_next_some()'
        let my_vote = Vote {
            commit_id: "commit_abc".to_string(),
            validator_key: "peer_xyz".to_string(),
            signature: vec![1, 2, 3, 4],
        };

        let msg = ArcadiaNetMessage::SubmitVote(my_vote);

        // 1. Sérialisation (Simulation de la conversion en octets pour le réseau)
        let payload =
            json::serialize_to_bytes(&msg).expect("Erreur de sérialisation du message réseau");

        // 2. Désérialisation (Simulation de 'json::deserialize_from_bytes' dans la boucle d'écoute)
        let parsed_msg = json::deserialize_from_bytes::<ArcadiaNetMessage>(&payload);

        assert!(
            parsed_msg.is_ok(),
            "La désérialisation du payload Gossipsub a échoué"
        );

        if let Ok(ArcadiaNetMessage::SubmitVote(v)) = parsed_msg {
            assert_eq!(v.commit_id, "commit_abc");
            assert_eq!(v.validator_key, "peer_xyz");
            assert_eq!(v.signature, vec![1, 2, 3, 4]);
        } else {
            panic!("Le message désérialisé ne correspond pas à la structure d'origine !");
        }
    }
}
