// FICHIER : crates/raise-core/src/blockchain/p2p/service.rs

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
use futures::StreamExt;

/// Structure regroupant les états du nœud pour injection externe
pub struct MentisNodeState {
    pub swarm_tx: AsyncChannel::Sender<MentisNetMessage>,
    pub shared_ledger: SharedRef<SyncMutex<Ledger>>,
    pub sync_engine: SharedRef<AsyncMutex<SyncEngine>>,
    pub pending_commits: SharedRef<AsyncMutex<PendingCommits>>,
    pub consensus: SharedRef<AsyncMutex<ConsensusEngine>>,
}

/// Point d'entrée unique pour initialiser et démarrer tout le réseau P2P Mentis.
pub async fn init_mentis_network(
    app_state: SharedRef<AppState>,
    storage_state: SharedRef<StorageEngine>,
) -> RaiseResult<MentisNodeState> {
    let local_key = P2pIdentity::Keypair::generate_ed25519();
    let local_peer_id = local_key.public().to_peer_id().to_string();

    // 🎯 RIGUEUR : On utilise le pattern matching et raise_error!
    let swarm = match create_swarm(local_key).await {
        Ok(s) => s,
        Err(e) => raise_error!("ERR_SWARM_INIT", error = e.to_string()),
    };

    let (swarm_tx, swarm_rx) = AsyncChannel::channel::<MentisNetMessage>(100);

    let shared_ledger = SharedRef::new(SyncMutex::new(Ledger::new()));
    let sync_engine = SharedRef::new(AsyncMutex::new(SyncEngine::new(shared_ledger.clone())));
    let pending_commits = SharedRef::new(AsyncMutex::new(PendingCommits::new()));
    let consensus = SharedRef::new(AsyncMutex::new(ConsensusEngine::new(1)));

    spawn_p2p_service(
        consensus.clone(),
        pending_commits.clone(),
        storage_state,
        app_state,
        swarm,
        swarm_rx,
        local_peer_id,
    );

    kernel_trace!("Mentis Network", "Swarm, Ledger et Consensus initialisés.");

    Ok(MentisNodeState {
        swarm_tx,
        shared_ledger,
        sync_engine,
        pending_commits,
        consensus,
    })
}

/// Démarre la boucle réseau P2P infinie en arrière-plan.
pub fn spawn_p2p_service(
    consensus_state: SharedRef<AsyncMutex<ConsensusEngine>>,
    pending_state: SharedRef<AsyncMutex<PendingCommits>>,
    storage_state: SharedRef<StorageEngine>,
    app_state: SharedRef<AppState>,
    mut swarm: P2pSwarm<MentisBehavior>,
    mut swarm_rx: AsyncChannel::Receiver<MentisNetMessage>,
    local_peer_id: String,
) {
    spawn_async_task(async move {
        loop {
            AgentAttention! {
                event = swarm.select_next_some() => {
                    if let P2pSwarmEvent::Behaviour(MentisBehaviorEvent::Gossipsub(P2pGossipSub::Event::Message { message, .. })) = event {
                        if let Ok(net_msg) = json::deserialize_from_bytes::<MentisNetMessage>(&message.data) {
                            match net_msg {
                                MentisNetMessage::AnnounceCommit(commit) if commit.verify() => {
                                    let mut pending = pending_state.lock().await;
                                    let mut engine = consensus_state.lock().await;

                                    pending.insert(commit.clone());
                                    engine.register_commit(&commit);

                                    let mut sig = vec![0xDE, 0xAD, 0xBE, 0xEF];
                                    sig.push((commit.id.len() % 255) as u8);

                                    let my_vote = Vote {
                                        commit_id: commit.id.clone(),
                                        voter: local_peer_id.clone(),
                                        signature: sig,
                                    };

                                    if let Ok(vote_data) = json::serialize_to_bytes(&MentisNetMessage::SubmitVote(my_vote)) {
                                        let topic = P2pGossipSub::IdentTopic::new("mentis-consensus");
                                        let _ = swarm.behaviour_mut().gossipsub.publish(topic, vote_data);
                                    }
                                },
                                MentisNetMessage::SubmitVote(vote) => {
                                    let mut engine = consensus_state.lock().await;

                                    if engine.process_incoming_vote(vote.clone()) {
                                        let mut pending = pending_state.lock().await;

                                        if let Some(final_commit) = pending.remove(&vote.commit_id) {
                                            let bridge = ArcadiaBridge::new(&storage_state, &app_state);
                                            let _ = bridge.process_new_commit(&final_commit).await;
                                            engine.finalize_validation(&final_commit.id);
                                        }
                                    }
                                },
                                _ => {}
                            }
                        }
                    }
                },
                Some(command) = swarm_rx.recv() => {
                    if let Ok(data) = json::serialize_to_bytes(&command) {
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
