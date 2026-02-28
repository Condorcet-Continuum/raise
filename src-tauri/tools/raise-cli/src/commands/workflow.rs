// FICHIER : src-tauri/tools/raise-cli/src/commands/workflow.rs

use clap::{Args, Subcommand};
use std::sync::Arc;
use tokio::sync::Mutex as AsyncMutex;

use raise::{
    user_error, user_info, user_success,
    utils::{
        config::AppConfig,
        io::{self},
        prelude::*,
        Utc,
    },
};

// Imports Cœur Raise
use raise::ai::orchestrator::AiOrchestrator;
use raise::json_db::collections::manager::CollectionsManager;
use raise::json_db::storage::{JsonDbConfig, StorageEngine};
use raise::model_engine::types::ProjectModel;
use raise::plugins::manager::PluginManager;

use raise::workflow_engine::{
    compiler::WorkflowCompiler, executor::WorkflowExecutor, mandate::Mandate,
    scheduler::WorkflowScheduler, ExecutionStatus, WorkflowInstance,
};

/// Pilotage avancé du Workflow Engine (Neuro-Symbolic & Sovereign)
#[derive(Args, Clone, Debug)]
pub struct WorkflowArgs {
    #[command(subcommand)]
    pub command: WorkflowCommands,
}

#[derive(Subcommand, Clone, Debug)]
pub enum WorkflowCommands {
    /// Soumet un Mandat (Politique de gouvernance) pour compilation et persistance
    SubmitMandate {
        /// Chemin vers le fichier de mandat (.json)
        path: String,
    },
    /// Met à jour une valeur de capteur (Jumeau Numérique local)
    SetSensor {
        /// Valeur f64 du capteur
        value: f64,
    },
    /// Démarre une nouvelle instance à partir d'un Mandat compilé
    Start {
        /// ID du workflow (généralement "wf_" + mandate_id)
        workflow_id: String,
    },
    /// Reprend un workflow en attente de validation (HITL)
    Resume {
        /// ID de l'instance
        instance_id: String,
        /// ID du nœud à débloquer
        node_id: String,
        /// Décision (true = approuvé, false = rejeté)
        #[arg(short, long)]
        approved: bool,
    },
    /// Affiche le statut détaillé d'une instance depuis la base de données
    Status { instance_id: String },
}

// --- HELPER D'INITIALISATION DU MOTEUR ---
// Permet au CLI de se connecter à la même base de données que le serveur Tauri
// et d'instancier son propre exécuteur IA.
async fn init_cli_engine() -> RaiseResult<(StorageEngine, WorkflowScheduler, String, String)> {
    let config = AppConfig::get();
    let db_root = config
        .get_path("PATH_RAISE_DOMAIN")
        .unwrap_or_else(|| std::path::PathBuf::from("./_system"));

    let storage = StorageEngine::new(JsonDbConfig::new(db_root));
    let domain = config.system_domain.clone();
    let db = config.system_db.clone();

    // Initialisation du moteur (Léger pour le CLI)
    let orch = match AiOrchestrator::new(ProjectModel::default(), None).await {
        Ok(instance) => instance,
        Err(e) => raise_error!(
            "ERR_AI_ORCHESTRATOR_INIT",
            error = e,
            context = json!({
                "action": "startup_ai_engine",
                "model_type": "ProjectModel::default",
                "hint": "Vérifiez la présence des fichiers de poids du modèle et la disponibilité de la VRAM."
            })
        ),
    };
    let pm = Arc::new(PluginManager::new(&storage, None));

    let executor = WorkflowExecutor::new(Arc::new(AsyncMutex::new(orch)), pm);
    let scheduler = WorkflowScheduler::new(executor);

    Ok((storage, scheduler, domain, db))
}

// --- POINT D'ENTRÉE PRINCIPAL ---
pub async fn handle(args: WorkflowArgs) -> RaiseResult<()> {
    match args.command {
        WorkflowCommands::SubmitMandate { path } => {
            user_info!(
                "MANDATE_LOAD_START",
                json!({ "path": path, "type": "config_source" })
            );
            let path_ref = io::Path::new(&path);

            if !io::exists(path_ref).await {
                raise_error!(
                    "FS_MANDATE_NOT_FOUND",
                    error = "File does not exist on disk",
                    context = json!({
                        "path": path,
                        "operation": "mandate_initialization",
                        "critical": true
                    })
                );
            }

            let content = match tokio::fs::read_to_string(path_ref).await {
                Ok(c) => c,
                Err(e) => raise_error!(
                    "ERR_FS_READ_CONTENT",
                    error = e,
                    context = json!({
                        "action": "read_file_to_string",
                        "path": path_ref.to_string_lossy(),
                        "hint": "Le fichier a peut-être été supprimé ou est utilisé par un autre processus."
                    })
                ),
            };

            let mandate: Mandate = match serde_json::from_str(&content) {
                Ok(m) => m,
                Err(e) => raise_error!(
                    "ERR_JSON_DESERIALIZE_MANDATE",
                    error = e,
                    context = json!({
                        "action": "parse_mandate_json",
                        "line": e.line(),
                        "column": e.column(),
                        "hint": "Le format du mandat ne correspond pas à la structure attendue. Vérifiez les types et les champs obligatoires."
                    })
                ),
            };

            // 1. Compilation pour vérifier la validité
            let definition = WorkflowCompiler::compile(&mandate);

            // 2. Connexion à la base de données pour persister le Mandat
            let (storage, _sched, domain, db) = init_cli_engine().await?;
            let manager = CollectionsManager::new(&storage, &domain, &db);

            let json_mandate = serde_json::to_value(&mandate).unwrap();

            // Persistance pour que le serveur Tauri puisse le trouver
            let Ok(_) = manager.insert_raw("mandates", &json_mandate).await else {
                raise_error!(
                    "MANDATE_SAVE_FAILED",
                    error = "Échec de l'insertion en base de données",
                    context = json!({
                        "collection": "mandates",
                        "mandate_id": json_mandate.get("id"),
                        "critical": true
                    })
                );
            };

            user_success!(
                "MANDATE_COMPILE_SUCCESS",
                json!({
                    "author": mandate.meta.author,
                    "version": mandate.meta.version,
                    "graph_id": definition.id,
                    "status": "persisted"
                })
            );
        }

        WorkflowCommands::Start { workflow_id } => {
            user_info!(
                "ENGINE_WORKFLOW_START",
                json!({
                    "workflow_id": workflow_id,
                    "mode": "initialization",
                    "timestamp": Utc::now().to_rfc3339()
                })
            );
            let (storage, mut scheduler, domain, db) = init_cli_engine().await?;
            let manager = CollectionsManager::new(&storage, &domain, &db);

            // Hack CLI: On "charge" la définition dans le scheduler courant en feignant le mandate_id
            let mandate_id = workflow_id.replace("wf_", "");
            scheduler.load_mission(&mandate_id, &manager).await?;

            let instance = scheduler.create_instance(&workflow_id, &manager).await?;
            user_success!(
                "INSTANCE_CREATION_SUCCESS",
                json!({
                    "instance_id": instance.id,
                    "status": "initialized",
                    "timestamp": Utc::now().to_rfc3339()
                })
            );
            let final_status = scheduler
                .execute_instance_loop(&instance.id, &manager)
                .await?;

            match final_status {
                ExecutionStatus::Completed => {
                    user_success!("WORKFLOW_COMPLETED");
                }
                ExecutionStatus::Paused => {
                    // Cas HITL (Human In The Loop) : on ajoute du contexte
                    user_info!(
                        "WORKFLOW_PAUSED_HITL",
                        json!({
                            "reason": "manual_validation_required",
                            "instance_id": instance.id
                        })
                    );
                }
                ExecutionStatus::Failed => {
                    user_error!(
                        "WORKFLOW_FAILED",
                        json!({ "final_status": format!("{:?}", final_status) })
                    );
                }
                _ => {
                    user_info!(
                        "WORKFLOW_STATUS_UPDATE",
                        json!({ "status": format!("{:?}", final_status) })
                    );
                }
            }
        }

        WorkflowCommands::Resume {
            instance_id,
            node_id,
            approved,
        } => {
            user_info!(
                "WORKFLOW_HITL_RESUME",
                json!({
                    "instance_id": instance_id,
                    "node_id": node_id,
                    "decision": if approved { "approved" } else { "rejected" },
                    "timestamp": Utc::now().to_rfc3339()
                })
            );
            let (storage, mut scheduler, domain, db) = init_cli_engine().await?;
            let manager = CollectionsManager::new(&storage, &domain, &db);

            // On a besoin de charger la définition pour exécuter la boucle
            let doc = manager
                .get_document("workflow_instances", &instance_id)
                .await
                .unwrap()
                .unwrap();
            let instance: WorkflowInstance = serde_json::from_value(doc).unwrap();
            let mandate_id = instance.workflow_id.replace("wf_", "");

            scheduler.load_mission(&mandate_id, &manager).await?;

            // Application de la décision humaine
            scheduler
                .resume_node(&instance_id, &node_id, approved, &manager)
                .await?;

            // Relance de la machine
            user_info!("ENGINE", "Relance de la boucle d'exécution...");
            let final_status = scheduler
                .execute_instance_loop(&instance_id, &manager)
                .await?;

            if final_status == ExecutionStatus::Completed {
                user_success!("DONE", "Workflow terminé avec succès !");
            } else {
                user_info!(
                    "WORKFLOW_STATUS_CHANGED",
                    json!({
                        "new_status": format!("{:?}", final_status),
                        "instance_id": instance_id,
                        "is_terminal": matches!(final_status, ExecutionStatus::Completed | ExecutionStatus::Failed)
                    })
                );
            }
        }

        WorkflowCommands::Status { instance_id } => {
            let config = AppConfig::get();
            let db_root = config
                .get_path("PATH_RAISE_DOMAIN")
                .unwrap_or_else(|| std::path::PathBuf::from("./_system"));
            let storage = StorageEngine::new(JsonDbConfig::new(db_root));
            let manager =
                CollectionsManager::new(&storage, &config.system_domain, &config.system_db);

            match manager
                .get_document("workflow_instances", &instance_id)
                .await
            {
                Ok(Some(doc)) => {
                    let instance: WorkflowInstance = serde_json::from_value(doc).unwrap();
                    // 1. Suivi du statut de la machine à états
                    user_info!(
                        "INSTANCE_STATE_SYNC",
                        json!({
                            "status": format!("{:?}", instance.status),
                            "instance_id": instance.id
                        })
                    );

                    // 2. Monitoring de la topologie active
                    user_info!(
                        "INSTANCE_NODES_SNAPSHOT",
                        json!({
                            "nodes": instance.node_states,
                            "count": instance.node_states.len()
                        })
                    );

                    // 3. Récupération du dernier événement de trace
                    if let Some(last_log) = instance.logs.last() {
                        user_info!("INSTANCE_LAST_EVENT", json!({ "log": last_log }));
                    }
                }
                _ => user_error!(
                    "INSTANCE_NOT_FOUND",
                    json!({
                        "instance_id": instance_id,
                        "action": "lookup_failure",
                        "severity": "medium"
                    })
                ),
            }
        }

        WorkflowCommands::SetSensor { value } => {
            // 1. Initialisation légère de l'accès DB (sans charger l'IA)
            let config = AppConfig::get();
            let db_root = config
                .get_path("PATH_RAISE_DOMAIN")
                .unwrap_or_else(|| std::path::PathBuf::from("./_system"));
            let storage = StorageEngine::new(JsonDbConfig::new(db_root));
            let manager =
                CollectionsManager::new(&storage, &config.system_domain, &config.system_db);

            // 2. Création de l'entité Jumeau Numérique
            let sensor_doc = serde_json::json!({
                "id": "vibration_z", // Identifiant unique du capteur
                "value": value,
                "updatedAt": Utc::now().to_rfc3339()
            });

            // 3. Persistance dans la collection 'digital_twin' (IPC par la donnée)
            // Note: On utilise insert_raw (qui fait un upsert si l'ID existe déjà dans votre implémentation)
            let Ok(_) = manager.insert_raw("digital_twin", &sensor_doc).await else {
                raise_error!(
                    "SENSOR_WRITE_FAILED",
                    error = "Échec d'écriture dans le jumeau numérique",
                    context = json!({
                        "collection": "digital_twin",
                        "sensor_id": sensor_doc.get("id"),
                        "critical": true
                    })
                );
            };
            user_success!(
                "SENSOR_UPDATED",
                json!({
                    "sensor_type": "vibration_z",
                    "value": value,
                    "collection": "digital_twin",
                    "status": "synchronized"
                })
            );
        }
    }
    Ok(())
}

// =========================================================================
// TESTS UNITAIRES ET D'INTÉGRATION CLI
// =========================================================================
// =========================================================================
// TESTS UNITAIRES ET D'INTÉGRATION CLI
// =========================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use raise::utils::config::test_mocks;
    use tempfile::tempdir;

    #[tokio::test]
    #[serial_test::serial]
    async fn test_cli_set_sensor_writes_to_db() {
        test_mocks::inject_mock_config();

        let args = WorkflowArgs {
            command: WorkflowCommands::SetSensor { value: 42.5 },
        };
        let result = handle(args).await;
        assert!(result.is_ok(), "La commande SetSensor a échoué");

        let config = AppConfig::get();
        let db_root = config
            .get_path("PATH_RAISE_DOMAIN")
            .unwrap_or_else(|| std::path::PathBuf::from("./_system"));
        let storage = StorageEngine::new(JsonDbConfig::new(db_root));
        let manager = CollectionsManager::new(&storage, &config.system_domain, &config.system_db);

        let doc = manager
            .get_document("digital_twin", "vibration_z")
            .await
            .expect("Erreur DB")
            .expect("Le capteur vibration_z n'a pas été trouvé en base");

        assert_eq!(doc["value"], 42.5, "La valeur en base ne correspond pas");
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn test_cli_submit_mandate_compiles_and_persists() {
        test_mocks::inject_mock_config();

        let dir = tempdir().unwrap();
        let mandate_path = dir.path().join("test_mandate.json");

        // 🎯 FIX : On donne un ID explicite pour pouvoir le récupérer directement
        let valid_mandate = serde_json::json!({
            "id": "mandate_cli_test_123",
            "name": { "fr": "Mandat de Test" },
            "meta": { "author": "CLI_Tester", "version": "1.0.0", "status": "ACTIVE" },
            "governance": { "strategy": "SAFETY_FIRST", "condorcetWeights": { "sec": 1.0 } },
            "hardLogic": { "vetos": [] },
            "observability": { "heartbeatMs": 100 }
        });
        std::fs::write(&mandate_path, valid_mandate.to_string()).unwrap();

        let args = WorkflowArgs {
            command: WorkflowCommands::SubmitMandate {
                path: mandate_path.to_string_lossy().to_string(),
            },
        };

        let result = handle(args).await;
        assert!(result.is_ok(), "La commande SubmitMandate a échoué");

        let config = AppConfig::get();
        let db_root = config.get_path("PATH_RAISE_DOMAIN").unwrap();
        let storage = StorageEngine::new(JsonDbConfig::new(db_root));
        let manager = CollectionsManager::new(&storage, &config.system_domain, &config.system_db);

        // 🎯 FIX : Utilisation de get_document au lieu du list_documents inexistant
        let doc = manager
            .get_document("mandates", "mandate_cli_test_123")
            .await
            .unwrap()
            .expect("Le mandat n'a pas été sauvegardé dans la collection 'mandates' !");

        assert_eq!(
            doc["meta"]["author"], "CLI_Tester",
            "Le mandat trouvé ne correspond pas"
        );
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn test_cli_submit_mandate_invalid_path_fails_safely() {
        // 1. Initialisation du contexte pour éviter les paniques hors-CUDA
        raise::utils::config::test_mocks::inject_mock_config();

        let fake_path = "path/to/nothing.yaml";

        // 🎯 FIX : On utilise la struct WorkflowArgs et l'énum WorkflowCommands
        // pour appeler la fonction 'handle' définie dans le module parent.
        let args = WorkflowArgs {
            command: WorkflowCommands::SubmitMandate {
                path: fake_path.to_string(),
            },
        };

        let result = super::handle(args).await;

        // 2. On vérifie que le CLI retourne une erreur propre au lieu de paniquer
        assert!(
            result.is_err(),
            "Le CLI ne doit pas paniquer mais retourner une Err pour un fichier manquant"
        );

        let err = result.unwrap_err();
        let err_msg = err.to_string();

        // 3. Validation du code d'erreur structuré RAISE
        // Dans handle, si le fichier manque, raise_error! "FS_MANDATE_NOT_FOUND" est appelée.
        assert!(
            err_msg.contains("FS_MANDATE_NOT_FOUND"),
            "L'erreur attendue était FS_MANDATE_NOT_FOUND. Reçu : {}",
            err_msg
        );
    }
}
