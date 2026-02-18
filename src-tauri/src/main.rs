// FICHIER : src-tauri/src/main.rs

#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use std::fs;
use std::path::Path;
use std::sync::{Arc, Mutex};

use futures::StreamExt;
use tauri::Manager;
use tokio::sync::Mutex as AsyncMutex;

use libp2p::gossipsub;
use libp2p::swarm::SwarmEvent;
use serde_json::Value;

// --- IMPORTS RAISE ---
use raise::ai::training::dataset;
use raise::blockchain::p2p::swarm::create_swarm;
use raise::blockchain::storage::chain::Ledger;
use raise::blockchain::sync::engine::SyncEngine;
use raise::blockchain::{ConnectionProfile, FabricClient, SharedFabricClient};
use raise::commands::{
    ai_commands, blockchain_commands, codegen_commands, cognitive_commands, genetics_commands,
    json_db_commands, model_commands, rules_commands, traceability_commands, utils_commands,
    workflow_commands,
};

// --- BRIDGE, CONSENSUS & P2P ---
use raise::blockchain::bridge::ArcadiaBridge;
use raise::blockchain::consensus::{ConsensusEngine, Vote};
use raise::blockchain::p2p::behavior::ArcadiaBehaviorEvent;
use raise::blockchain::p2p::protocol::ArcadiaNetMessage;

// --- IMPORT IA NATIF ---
use raise::ai::llm::candle_engine::CandleLlmEngine;
use raise::ai::llm::NativeLlmState;

use raise::json_db::jsonld::VocabularyRegistry;
use raise::json_db::migrations::migrator::Migrator;
use raise::json_db::migrations::{Migration, MigrationStep};
use raise::json_db::storage::{JsonDbConfig, StorageEngine};

use raise::plugins::manager::PluginManager;

// Structures d'état
use raise::commands::ai_commands::AiState;
use raise::commands::workflow_commands::WorkflowStore;
use raise::workflow_engine::scheduler::WorkflowScheduler;

pub use raise::model_engine::types::ProjectModel;
use raise::AppState;

use raise::ai::orchestrator::AiOrchestrator;
use raise::graph_store::GraphStore;
use raise::model_engine::loader::ModelLoader;

use raise::commands::ai_commands::DlState;

use raise::spatial_engine;

#[allow(clippy::await_holding_lock)]
fn main() {
    if let Err(e) = raise::utils::config::AppConfig::init() {
        eprintln!("❌ Erreur fatale de configuration : {}", e);
        std::process::exit(1);
    }
    println!("🚀 Démarrage de RAISE...");
    raise::utils::init_logging();
    // 3. VÉRIFICATION (SANS ERREUR DE SYNTAXE)
    let _config = raise::utils::config::AppConfig::get();

    tauri::Builder::default()
        .manage(NativeLlmState(std::sync::Mutex::new(None)))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .setup(|app| {
            // 2. CONFIG DOMAINE & STOCKAGE
            let app_config = raise::utils::config::AppConfig::get();
            let db_root = app_config.get_path("PATH_RAISE_DOMAIN")
                .expect("❌ ERREUR FATALE: PATH_RAISE_DOMAIN introuvable dans la configuration !");
            if !db_root.exists() {
                fs::create_dir_all(&db_root)?;
            }
            let config = JsonDbConfig::new(db_root.clone());
            let storage = StorageEngine::new(config.clone());

            // On utilise les espaces par défaut définis dans le JSON !
            let default_space = &app_config.default_domain;
            let default_db = &app_config.default_db;

            // 3. CHARGEMENT ONTOLOGIES (Depuis le dossier de domaine)
            // Construction du chemin : ~/raise_domain/mbse2/_system/schemas/v1/arcadia/@context
            let ontology_path = db_root
                .join(default_space)
                .join(default_db)
                .join("schemas/v1/arcadia/@context");

            //load_arcadia_ontologies(&ontology_path) ;
            tauri::async_runtime::spawn(async move {
                load_arcadia_ontologies(&ontology_path).await;
            });
            // 4. GRAPH STORE
            let graph_path = db_root.join("graph_store");
            let graph_store_result =
                tauri::async_runtime::block_on(async { GraphStore::new(graph_path).await });

            if let Ok(store) = graph_store_result {
                app.manage(store);
                println!("✅ [GraphStore] Base Graphe principale chargée.");
            } else {
                eprintln!("❌ [GraphStore] Echec chargement base graphe.");
            }

            // 5. MIGRATIONS
            let _ = tauri::async_runtime::block_on(run_app_migrations(
                &storage,
                default_space,
                default_db,
            ));

            let plugin_mgr = Arc::new(PluginManager::new(&storage, None));

            // 6. INJECTION DES ÉTATS
            app.manage(config);
            let storage_engine = storage.clone();
            app.manage(storage);
            app.manage(plugin_mgr.clone());

            let app_state = Arc::new(AppState {
                model: std::sync::Mutex::new(ProjectModel::default()),
            });
            app.manage(app_state.clone());

            app.manage(AsyncMutex::new(WorkflowStore::default()));
            app.manage(AiState::new(None));
            app.manage(DlState::new());

            let app_handle = app.handle();
            raise::blockchain::ensure_innernet_state(app_handle, "default");

            // --- INITIALISATION FABRIC ---
            let default_fabric_profile = ConnectionProfile {
                name: "pending".into(),
                version: "1.0.0".into(),
                client: raise::blockchain::fabric::config::ClientConfig {
                    organization: "none".into(),
                    connection: None,
                },
                organizations: std::collections::HashMap::new(),
                peers: std::collections::HashMap::new(),
                certificate_authorities: std::collections::HashMap::new(),
            };
            app.manage(
                Mutex::new(FabricClient::from_config(default_fabric_profile)) as SharedFabricClient,
            );

            // --- INITIALISATION RÉSEAU ARCADIA ---
            let local_key = libp2p::identity::Keypair::generate_ed25519();
            let local_peer_id = local_key.public().to_peer_id().to_string();
            let swarm_res = tauri::async_runtime::block_on(async { create_swarm(local_key).await });

            if let Ok(swarm) = swarm_res {
                app.manage(AsyncMutex::new(swarm));
                app.manage(Mutex::new(Ledger::new()));
                app.manage(Mutex::new(SyncEngine::new()));

                let innernet = raise::blockchain::innernet_state(app.handle());
                let peers_res = tauri::async_runtime::block_on(async {
                    innernet.lock().unwrap().list_peers().await
                });

                if let Ok(peers) = peers_res {
                    let consensus = ConsensusEngine::new(&peers, 0.5);
                    app.manage(AsyncMutex::new(consensus));
                    println!("✅ [Arcadia] Swarm, Ledger et Consensus initialisés.");
                } else {
                    eprintln!("⚠️ [Arcadia] Impossible de récupérer les pairs VPN.");
                }
            } else {
                eprintln!("❌ [Arcadia] Échec du démarrage du réseau P2P.");
            }

            // --- BACKGROUND: IA NATIF ---
            let native_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                match CandleLlmEngine::new() {
                    Ok(engine) => {
                        let state = native_handle.state::<NativeLlmState>();
                        *state.0.lock().unwrap() = Some(engine);
                        println!("✅ [Background] Moteur IA Natif prêt !");
                    }
                    Err(e) => {
                        eprintln!("❌ [Background] Echec chargement IA Natif : {}", e);
                    }
                }
            });

            // --- BACKGROUND: ORCHESTRATEUR IA ---
            let app_handle_clone = app.handle().clone();
            let plugin_mgr_for_wf = plugin_mgr.clone();

            tauri::async_runtime::spawn(async move {
                let global_cfg = raise::utils::config::AppConfig::get();

                // 1. Récupération de l'URL LLM depuis ai_engines
                let llm_url = global_cfg.ai_engines.get("primary_local")
                    .and_then(|engine| engine.api_url.clone())
                    .unwrap_or_else(|| "http://127.0.0.1:8081".to_string());

                // 2. Récupération de l'URL Qdrant GRPC depuis services
                let qdrant_url = global_cfg.services.get("qdrant_grpc")
                    .map(|s| format!("{}://{}:{}", s.protocol.as_deref().unwrap_or("http"), s.host, s.port))
                    .unwrap_or_else(|| "http://127.0.0.1:6334".to_string());

                let storage_state = app_handle_clone.state::<StorageEngine>();

                // 3. On utilise l'espace dynamique
                let _ = ModelLoader::from_engine(
                    storage_state.inner(),
                    &global_cfg.default_domain,
                    &global_cfg.default_db
                );

                let storage_state = app_handle_clone.state::<StorageEngine>();
                // On utilise le même espace que défini plus haut (mbse2)
                let loader = ModelLoader::from_engine(storage_state.inner(), "mbse2", "_system");

                if let Ok(model) = loader.load_full_model().await {
                    let storage_arc = Arc::new(storage_state.inner().clone());

                    match AiOrchestrator::new(model, &qdrant_url, &llm_url, Some(storage_arc)).await
                    {
                        Ok(orchestrator) => {
                            let shared_orch = Arc::new(AsyncMutex::new(orchestrator));
                            let ai_state = app_handle_clone.state::<AiState>();
                            *ai_state.0.lock().await = Some(shared_orch.clone());
                            let wf_state = app_handle_clone.state::<AsyncMutex<WorkflowStore>>();
                            let mut wf_store = wf_state.lock().await;
                            wf_store.scheduler =
                                Some(WorkflowScheduler::new(shared_orch, plugin_mgr_for_wf));

                            println!("✅ [RAISE] Orchestrateur IA opérationnel.");
                        }
                        Err(e) => eprintln!("❌ Erreur Fatale Orchestrator: {}", e),
                    }
                } else {
                    eprintln!("⚠️ [IA] Impossible de charger le modèle symbolique initial.");
                }
            });

            // --- BOUCLE P2P ---
            let swarm_handle = app.handle().clone();
            let storage_for_p2p = storage_engine;
            let app_state_for_p2p = app_state;
            let local_id_for_vote = local_peer_id;

            tauri::async_runtime::spawn(async move {
                let swarm_state = swarm_handle.state::<AsyncMutex<
                    libp2p::Swarm<raise::blockchain::p2p::behavior::ArcadiaBehavior>,
                >>();
                let consensus_state = swarm_handle.state::<AsyncMutex<ConsensusEngine>>();
                let mut swarm = swarm_state.lock().await;

                loop {
                    tokio::select! {
                        event = swarm.select_next_some() => {
                            // CORRECTION CLIPPY: Remplacement de `match` par `if let` pour un cas unique
                            if let SwarmEvent::Behaviour(ArcadiaBehaviorEvent::Gossipsub(gossipsub::Event::Message { message, .. })) = event {
                                if let Ok(net_msg) = serde_json::from_slice::<ArcadiaNetMessage>(&message.data) {
                                    let mut engine = consensus_state.lock().await;
                                    match net_msg {
                                        ArcadiaNetMessage::AnnounceCommit(commit) => {
                                            if engine.verify_authority(&commit) {
                                                let _ = engine.register_proposal(commit.clone());
                                                let my_vote = Vote {
                                                    commit_id: commit.id.clone(),
                                                    validator_key: local_id_for_vote.clone(),
                                                    signature: vec![1, 0, 1, 0],
                                                };
                                                if let Ok(vote_data) = serde_json::to_vec(&ArcadiaNetMessage::SubmitVote(my_vote)) {
                                                    let topic = gossipsub::IdentTopic::new("arcadia-consensus");
                                                    let _ = swarm.behaviour_mut().gossipsub.publish(topic, vote_data);
                                                }
                                            }
                                        },
                                        ArcadiaNetMessage::SubmitVote(vote) => {
                                            if let Ok(Some(final_commit)) = engine.process_vote(vote) {
                                                let bridge = ArcadiaBridge::new(&storage_for_p2p, &app_state_for_p2p);
                                                let _ = bridge.process_new_commit(&final_commit).await;
                                                engine.finalize_commit(&final_commit.id);
                                            }
                                        },
                                        _ => {}
                                    }
                                }
                            }
                        }
                    }
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            json_db_commands::jsondb_create_db,
            json_db_commands::jsondb_drop_db,
            json_db_commands::jsondb_create_collection,
            json_db_commands::jsondb_list_collections,
            json_db_commands::jsondb_drop_collection,
            json_db_commands::jsondb_create_index,
            json_db_commands::jsondb_drop_index,
            json_db_commands::jsondb_insert_document,
            json_db_commands::jsondb_get_document,
            json_db_commands::jsondb_update_document,
            json_db_commands::jsondb_delete_document,
            json_db_commands::jsondb_list_all,
            json_db_commands::jsondb_execute_query,
            json_db_commands::jsondb_execute_sql,
            json_db_commands::jsondb_evaluate_draft,
            json_db_commands::jsondb_init_demo_rules,
            model_commands::load_project_model,
            rules_commands::dry_run_rule,
            rules_commands::validate_model,
            ai_commands::ai_chat,
            ai_commands::ai_reset,
            ai_commands::ask_native_llm,
            ai_commands::ai_learn_text,
            ai_commands::init_dl_model,
            ai_commands::run_dl_prediction,
            ai_commands::train_dl_step,
            ai_commands::save_dl_model,
            ai_commands::load_dl_model,
            dataset::ai_export_dataset,
            cognitive_commands::cognitive_load_plugin,
            cognitive_commands::cognitive_run_plugin,
            cognitive_commands::cognitive_list_plugins,
            blockchain_commands::fabric_ping,
            blockchain_commands::fabric_submit_transaction,
            blockchain_commands::fabric_query_transaction,
            blockchain_commands::fabric_get_history,
            blockchain_commands::vpn_network_status,
            blockchain_commands::vpn_connect,
            blockchain_commands::vpn_disconnect,
            blockchain_commands::vpn_list_peers,
            blockchain_commands::vpn_add_peer,
            blockchain_commands::vpn_ping_peer,
            blockchain_commands::vpn_check_installation,
            blockchain_commands::arcadia_broadcast_mutation,
            blockchain_commands::arcadia_get_sync_status,
            blockchain_commands::arcadia_get_ledger_info,
            genetics_commands::run_architecture_optimization,
            genetics_commands::debug_genetics_ping,
            codegen_commands::generate_source_code,
            traceability_commands::analyze_impact,
            traceability_commands::run_compliance_audit,
            traceability_commands::get_traceability_matrix,
            traceability_commands::get_element_neighbors,
            utils_commands::get_app_info,
            workflow_commands::submit_mandate,
            workflow_commands::register_workflow,
            workflow_commands::start_workflow,
            workflow_commands::resume_workflow,
            workflow_commands::get_workflow_state,
            workflow_commands::set_sensor_value,
            spatial_engine::get_spatial_topology
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

async fn run_app_migrations(storage: &StorageEngine, space: &str, db: &str) -> anyhow::Result<()> {
    let migrator = Migrator::new(storage, space, db);
    let migrations = vec![
        Migration {
            id: "init_001_core_collections".to_string(),
            version: "1.0.0".to_string(),
            description: "Init".to_string(),
            up: vec![
                MigrationStep::CreateCollection {
                    name: "articles".to_string(),
                    schema: Value::Null,
                },
                MigrationStep::CreateCollection {
                    name: "systems".to_string(),
                    schema: Value::Null,
                },
                MigrationStep::CreateCollection {
                    name: "exchange_items".to_string(),
                    schema: Value::Null,
                },
            ],
            down: vec![],
            applied_at: None,
        },
        Migration {
            id: "idx_001_articles_title".to_string(),
            version: "1.1.0".to_string(),
            description: "Idx title".to_string(),
            up: vec![MigrationStep::CreateIndex {
                collection: "articles".to_string(),
                fields: vec!["title".to_string()],
            }],
            down: vec![],
            applied_at: None,
        },
    ];
    migrator.run_migrations(migrations).await?;
    Ok(())
}

// --- LOGIQUE DE CHARGEMENT DES ONTOLOGIES ---

async fn load_arcadia_ontologies(ontology_root: &Path) {
    let registry = VocabularyRegistry::global();
    let layers = ["oa", "sa", "la", "pa", "epbs", "data", "transverse"];

    println!(
        "📂 [Ontology] Démarrage du chargement depuis {:?}",
        ontology_root
    );

    for layer in layers {
        let path = ontology_root.join(format!("{}.jsonld", layer));

        if path.exists() {
            if let Err(e) = registry.load_layer_from_file(layer, &path).await {
                eprintln!(
                    "⚠️ [Ontology] Erreur lors du chargement de {}: {}",
                    layer, e
                );
            } else {
                println!("✅ [Ontology] Couche '{}' chargée avec succès.", layer);
            }
        } else {
            eprintln!(
                "⚠️ [Ontology] Fichier manquant pour la couche '{}' : {:?}",
                layer, path
            );
        }
    }
}

// ============================================================================
// TESTS UNITAIRES
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_load_ontologies_from_directory_success() {
        let dir = tempdir().unwrap();
        let path = dir.path();

        let oa_content = r#"{ "@context": { "oa": "http://oa#" } }"#;
        let transverse_content = r#"{ "@context": { "lib": "http://lib#" } }"#;

        let oa_path = path.join("oa.jsonld");
        let trans_path = path.join("transverse.jsonld");

        let mut f_oa = File::create(&oa_path).unwrap();
        write!(f_oa, "{}", oa_content).unwrap();

        let mut f_trans = File::create(&trans_path).unwrap();
        write!(f_trans, "{}", transverse_content).unwrap();

        load_arcadia_ontologies(path).await;

        let registry = VocabularyRegistry::global();
        assert!(registry.get_context_for_layer("oa").is_some());
        assert!(registry.get_context_for_layer("transverse").is_some());

        let _ctx_sa = registry.get_context_for_layer("sa");
    }

    #[tokio::test]
    async fn test_migrations_list_integrity() {
        let dir = tempdir().unwrap();
        let config = JsonDbConfig::new(dir.path().to_path_buf());
        let storage = StorageEngine::new(config);
        let res = run_app_migrations(&storage, "test_space", "test_db").await;
        assert!(res.is_ok());
    }
}
