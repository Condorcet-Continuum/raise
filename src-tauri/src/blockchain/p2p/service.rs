// FICHIER : src-tauri/src/blockchain/p2p/service.rs

use futures::StreamExt; // Gardé car nécessaire pour le .select_next_some() des streams
use tauri::{AppHandle, Manager};

use crate::blockchain::bridge::ArcadiaBridge;
use crate::blockchain::consensus::pending::PendingCommits;
use crate::blockchain::consensus::{vote::Vote, ConsensusEngine};
use crate::blockchain::p2p::behavior::MentisBehavior;
use crate::blockchain::p2p::behavior::MentisBehaviorEvent;
use crate::blockchain::p2p::protocol::MentisNetMessage;
use crate::blockchain::p2p::swarm::create_swarm;
use crate::blockchain::storage::chain::Ledger;
use crate::blockchain::sync::engine::SyncEngine;
use crate::json_db::storage::StorageEngine;
use crate::utils::prelude::*;
use crate::AppState;

/// Point d'entrée unique pour initialiser et démarrer tout le réseau P2P Mentis.
#[allow(clippy::await_holding_lock)]
pub fn init_mentis_network(app_handle: AppHandle) {
    // 1. Génération de l'identité locale via la façade
    let local_key = P2pIdentity::Keypair::generate_ed25519();
    let local_peer_id = local_key.public().to_peer_id().to_string();

    // 2. Création du Swarm
    let swarm_res = tauri::async_runtime::block_on(async { create_swarm(local_key).await });

    if let Ok(swarm) = swarm_res {
        // 3. Création du canal MPSC pour communiquer avec l'UI
        let (swarm_tx, swarm_rx) = AsyncChannel::channel::<MentisNetMessage>(100);

        // 4. Injection des états dans Tauri
        let shared_ledger = SharedRef::new(SyncMutex::new(Ledger::new()));
        let sync_engine = SyncEngine::new(shared_ledger.clone());

        app_handle.manage(swarm_tx);
        app_handle.manage(shared_ledger);
        app_handle.manage(AsyncMutex::new(sync_engine));
        app_handle.manage(AsyncMutex::new(PendingCommits::new()));

        // 5. Configuration du Consensus (Quorum par défaut pour le bootstrap)
        let default_quorum = 1;

        let consensus = ConsensusEngine::new(default_quorum);
        app_handle.manage(AsyncMutex::new(consensus));

        // 6. Lancement de la boucle infinie en arrière-plan
        spawn_p2p_service(app_handle.clone(), swarm, swarm_rx, local_peer_id);

        kernel_trace!("Mentis Network", "Swarm, Ledger et Consensus initialisés.");
    } else {
        kernel_fatal!(
            "Démarrage Réseau P2P",
            "p2p Swarm Service",
            "Échec de l'initialisation du Swarm (Network Stack)"
        );
    }
}

/// Démarre la boucle réseau P2P infinie en arrière-plan.
pub fn spawn_p2p_service(
    app_handle: AppHandle,
    mut swarm: P2pSwarm<MentisBehavior>, // 🎯 FIX: Utilisation de l'alias P2pSwarm
    mut swarm_rx: AsyncChannel::Receiver<MentisNetMessage>,
    local_peer_id: String,
) {
    // 🎯 FIX: Utilisation de l'alias de tâche asynchrone du prelude
    spawn_async_task(async move {
        // Récupération des pointeurs partagés
        let consensus_state = app_handle.state::<AsyncMutex<ConsensusEngine>>();
        let pending_state = app_handle.state::<AsyncMutex<PendingCommits>>();
        let storage_state = app_handle.state::<StorageEngine>();
        let app_state = app_handle.state::<SharedRef<AppState>>();

        loop {
            // 🎯 AgentAttention! : Alias de tokio::select!
            AgentAttention! {
                // 1. ÉCOUTE DU RÉSEAU P2P (Entrant)
                event = swarm.select_next_some() => {
                    // 🎯 FIX: Utilisation de l'alias P2pGossipSub
                    if let P2pSwarmEvent::Behaviour(MentisBehaviorEvent::Gossipsub(P2pGossipSub::Event::Message { message, .. })) = event {

                        if let Ok(net_msg) = json::deserialize_from_bytes::<MentisNetMessage>(&message.data) {

                            match net_msg {
                                // A. Réception d'une nouvelle mutation sur le réseau
                                MentisNetMessage::AnnounceCommit(commit) => {
                                    // Vérification cryptographique absolue du bloc
                                    if commit.verify() {
                                        let mut pending = pending_state.lock().await;
                                        let mut engine = consensus_state.lock().await;

                                        // On stocke le gros bloc dans le buffer et on ouvre le vote
                                        pending.insert(commit.clone());
                                        engine.register_commit(&commit);

                                        // On génère la signature de notre vote (Mock adapté au verify_signature)
                                        let mut sig = vec![0xDE, 0xAD, 0xBE, 0xEF];
                                        sig.push((commit.id.len() % 255) as u8);

                                        let my_vote = Vote {
                                            commit_id: commit.id.clone(),
                                            voter: local_peer_id.clone(),
                                            signature: sig,
                                        };

                                        // On publie notre vote à tous les pairs
                                        if let Ok(vote_data) = json::serialize_to_bytes(&MentisNetMessage::SubmitVote(my_vote)) {
                                            // 🎯 FIX: Utilisation de l'alias P2pGossipSub
                                            let topic = P2pGossipSub::IdentTopic::new("mentis-consensus");
                                            let _ = swarm.behaviour_mut().gossipsub.publish(topic, vote_data);
                                        }
                                    }
                                },
                                // B. Réception du vote d'un autre nœud
                                MentisNetMessage::SubmitVote(vote) => {
                                    let mut engine = consensus_state.lock().await;

                                    // process_incoming_vote vérifie la signature et gère l'Anti-Sybil
                                    if engine.process_incoming_vote(vote.clone()) {
                                        // 🎯 QUORUM ATTEINT !
                                        let mut pending = pending_state.lock().await;

                                        // On récupère le bloc complet dans le buffer
                                        if let Some(final_commit) = pending.remove(&vote.commit_id) {
                                            // On écrit dans la JSON-DB locale
                                            let bridge = ArcadiaBridge::new(storage_state.inner(), app_state.inner());
                                            let _ = bridge.process_new_commit(&final_commit).await;

                                            // On nettoie le consensus
                                            engine.finalize_validation(&final_commit.id);
                                        }
                                    }
                                },
                                _ => {}
                            }
                        }
                    }
                },

                // 2. ÉCOUTE DES COMMANDES INTERNES (Client lourd -> Réseau)
                Some(command) = swarm_rx.recv() => {
                    if let Ok(data) = json::serialize_to_bytes(&command) {
                        // 🎯 FIX: Utilisation de l'alias P2pGossipSub
                        let topic = P2pGossipSub::IdentTopic::new("mentis-consensus");
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
    use crate::blockchain::consensus::vote::Vote;
    use crate::blockchain::p2p::protocol::MentisNetMessage;
    use crate::blockchain::storage::commit::MentisCommit;

    #[async_test]
    async fn test_p2p_channel_communication() {
        let (tx, mut rx) = AsyncChannel::channel::<MentisNetMessage>(100);

        let commit = MentisCommit {
            id: "test_commit_123".to_string(),
            parent_hash: None,
            author: "local_peer".to_string(),
            timestamp: UtcClock::now(),
            mutations: vec![],
            merkle_root: "root_hash".to_string(),
            signature: vec![],
        };

        let msg = MentisNetMessage::AnnounceCommit(commit);

        assert!(
            tx.send(msg).await.is_ok(),
            "L'envoi dans le canal P2P a échoué"
        );

        let received = rx.recv().await;
        assert!(received.is_some(), "Le service P2P n'a rien reçu");

        if let Some(MentisNetMessage::AnnounceCommit(c)) = received {
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
        let my_vote = Vote {
            commit_id: "commit_abc".to_string(),
            voter: "peer_xyz".to_string(),
            signature: vec![1, 2, 3, 4],
        };

        let msg = MentisNetMessage::SubmitVote(my_vote);
        let payload =
            json::serialize_to_bytes(&msg).expect("Erreur de sérialisation du message réseau");
        let parsed_msg = json::deserialize_from_bytes::<MentisNetMessage>(&payload);

        assert!(
            parsed_msg.is_ok(),
            "La désérialisation du payload Gossipsub a échoué"
        );

        if let Ok(MentisNetMessage::SubmitVote(v)) = parsed_msg {
            assert_eq!(v.commit_id, "commit_abc");
            assert_eq!(v.voter, "peer_xyz");
            assert_eq!(v.signature, vec![1, 2, 3, 4]);
        } else {
            panic!("Le message désérialisé ne correspond pas à la structure d'origine !");
        }
    }
}
