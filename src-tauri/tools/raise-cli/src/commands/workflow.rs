// FICHIER : src-tauri/tools/raise-cli/src/commands/workflow.rs

use clap::{Args, Subcommand};
use raise::utils::prelude::*;
// Imports Cœur Raise
use raise::ai::orchestrator::AiOrchestrator;
use raise::json_db::collections::manager::CollectionsManager;
use raise::model_engine::types::ProjectModel;
use raise::plugins::manager::PluginManager;

use raise::workflow_engine::{
    compiler::WorkflowCompiler, executor::WorkflowExecutor, mandate::Mandate,
    scheduler::WorkflowScheduler, ExecutionStatus, WorkflowInstance,
};

// 🎯 NOUVEAU : Import du contexte global CLI
use crate::CliContext;

/// Pilotage avancé du Workflow Engine (Neuro-Symbolic & Sovereign)
#[derive(Args, Clone, Debug)]
pub struct WorkflowArgs {
    #[command(subcommand)]
    pub command: WorkflowCommands,
}

#[derive(Subcommand, Clone, Debug)]
pub enum WorkflowCommands {
    /// Importe un Mandat (Politique de gouvernance) en base de données
    SubmitMandate {
        /// Chemin vers le fichier de mandat (.json)
        path: String,
    },
    /// Compile une mission métier en un graphe d'exécution
    CompileMission {
        /// ID de la mission
        mission_id: String,
    },
    /// Met à jour une valeur de capteur (Jumeau Numérique local)
    SetSensor {
        /// Valeur f64 du capteur
        value: f64,
    },
    /// Démarre une nouvelle instance à partir d'un graphe compilé
    Start {
        /// ID de la mission
        mission_id: String,
        /// ID du workflow compilé
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
// 🎯 On utilise le contexte existant sans recréer de StorageEngine !
async fn init_cli_engine(ctx: &CliContext) -> RaiseResult<WorkflowScheduler> {
    // Initialisation du moteur IA via le contexte
    let manager = CollectionsManager::new(&ctx.storage, &ctx.active_domain, &ctx.active_db);
    let orch =
        match AiOrchestrator::new(ProjectModel::default(), &manager, ctx.storage.clone()).await {
            Ok(instance) => instance,
            Err(e) => raise_error!(
                "ERR_AI_ORCHESTRATOR_INIT",
                error = e,
                context = json_value!({
                    "action": "startup_ai_engine",
                    "hint": "Vérifiez la VRAM et les poids du modèle."
                })
            ),
        };

    // Utilisation du storage global partagé
    let pm = SharedRef::new(PluginManager::new(&ctx.storage, None));
    let executor = WorkflowExecutor::new(SharedRef::new(AsyncMutex::new(orch)), pm);

    Ok(WorkflowScheduler::new(executor))
}

// --- POINT D'ENTRÉE PRINCIPAL ---
// 🎯 La signature intègre le CliContext
pub async fn handle(args: WorkflowArgs, ctx: CliContext) -> RaiseResult<()> {
    // 🎯 Heartbeat automatique
    let _ = ctx.session_mgr.touch().await;

    match args.command {
        WorkflowCommands::SubmitMandate { path } => {
            user_info!(
                "MANDATE_LOAD_START",
                json_value!({ "path": path, "type": "config_source" })
            );
            let path_ref = Path::new(&path);

            if !fs::exists_async(path_ref).await {
                raise_error!(
                    "FS_MANDATE_NOT_FOUND",
                    error = "File does not exist on disk",
                    context = json_value!({
                        "path": path,
                        "operation": "mandate_initialization",
                        "critical": true
                    })
                );
            }

            let content = match fs::read_to_string_async(path_ref).await {
                Ok(c) => c,
                Err(e) => raise_error!(
                    "ERR_FS_READ_CONTENT",
                    error = e,
                    context = json_value!({
                        "action": "read_file_to_string",
                        "path": path_ref.to_string_lossy(),
                        "hint": "Le fichier a peut-être été supprimé ou est utilisé par un autre processus."
                    })
                ),
            };

            let mandate: Mandate = match json::deserialize_from_str(&content) {
                Ok(m) => m,
                Err(e) => raise_error!(
                    "ERR_JSON_DESERIALIZE_MANDATE",
                    error = e,
                    context = json_value!({
                        "action": "parse_mandate_json",
                        "error_details": e.to_string(),
                        "hint": "Le format du mandat ne correspond pas à la structure attendue. Vérifiez les types et les champs obligatoires."
                    })
                ),
            };

            // 1. Connexion à la base de données via le contexte !
            let manager = CollectionsManager::new(&ctx.storage, &ctx.active_domain, &ctx.active_db);

            // 2. Persistance (La compilation se fait désormais via la commande CompileMission)
            let json_mandate = json::serialize_to_value(&mandate).unwrap();
            manager.upsert_document("mandates", json_mandate).await?;

            user_success!(
                "MANDATE_IMPORT_SUCCESS",
                json_value!({
                    "mandator_id": mandate.meta.mandator_id,
                    "version": mandate.meta.version,
                    "active_domain": ctx.active_domain,
                    "active_user": ctx.active_user,
                    "status": "persisted",
                })
            );
        }

        WorkflowCommands::CompileMission { mission_id } => {
            user_info!(
                "MISSION_COMPILE_START",
                json_value!({ "mission_id": mission_id })
            );

            let manager = CollectionsManager::new(&ctx.storage, &ctx.active_domain, &ctx.active_db);
            let definition = WorkflowCompiler::compile(&manager, &mission_id).await?;

            user_success!(
                "MISSION_COMPILE_SUCCESS",
                json_value!({
                    "mission_id": mission_id,
                    "graph_handle": definition.handle,
                    "status": "compiled",
                })
            );
        }

        WorkflowCommands::Start {
            mission_id,
            workflow_id,
        } => {
            user_info!(
                "ENGINE_WORKFLOW_START",
                json_value!({
                    "mission_id": mission_id,
                    "workflow_id": workflow_id,
                    "mode": "initialization",
                    "timestamp": UtcClock::now().to_rfc3339()
                })
            );

            let mut scheduler = init_cli_engine(&ctx).await?;
            let manager = CollectionsManager::new(&ctx.storage, &ctx.active_domain, &ctx.active_db);

            scheduler.load_mission(&mission_id, &manager).await?;

            let instance = scheduler
                .create_instance(&mission_id, &workflow_id, &manager)
                .await?;
            user_success!(
                "INSTANCE_CREATION_SUCCESS",
                json_value!({
                    "instance_handle": instance.handle,
                    "status": "initialized",
                    "timestamp": UtcClock::now().to_rfc3339()
                })
            );
            let final_status = scheduler
                .execute_instance_loop(&instance.handle, &manager)
                .await?;

            match final_status {
                ExecutionStatus::Completed => {
                    user_success!("WORKFLOW_COMPLETED", json_value!({"status": "finished"}));
                }
                ExecutionStatus::Paused => {
                    user_info!(
                        "WORKFLOW_PAUSED_HITL",
                        json_value!({
                            "reason": "manual_validation_required",
                            "instance_handle": instance.handle
                        })
                    );
                }
                ExecutionStatus::Failed => {
                    user_error!(
                        "WORKFLOW_FAILED",
                        json_value!({ "final_status": format!("{:?}", final_status) })
                    );
                }
                _ => {
                    user_info!(
                        "WORKFLOW_STATUS_UPDATE",
                        json_value!({ "status": format!("{:?}", final_status) })
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
                json_value!({
                    "instance_id": instance_id,
                    "node_id": node_id,
                    "decision": if approved { "approved" } else { "rejected" },
                    "timestamp": UtcClock::now().to_rfc3339()
                })
            );

            let mut scheduler = init_cli_engine(&ctx).await?;
            let manager = CollectionsManager::new(&ctx.storage, &ctx.active_domain, &ctx.active_db);

            let doc = manager
                .get_document("workflow_instances", &instance_id)
                .await
                .unwrap()
                .unwrap();
            let instance: WorkflowInstance = json::deserialize_from_value(doc).unwrap();
            let mission_id = instance.mission_id.clone();

            scheduler.load_mission(&mission_id, &manager).await?;

            scheduler
                .resume_node(&instance_id, &node_id, approved, &manager)
                .await?;

            user_info!(
                "ENGINE",
                json_value!({"action": "Relance de la boucle d'exécution..."})
            );
            let final_status = scheduler
                .execute_instance_loop(&instance_id, &manager)
                .await?;

            if final_status == ExecutionStatus::Completed {
                user_success!(
                    "DONE",
                    json_value!({"status": "Workflow terminé avec succès !"})
                );
            } else {
                user_info!(
                    "WORKFLOW_STATUS_CHANGED",
                    json_value!({
                        "new_status": format!("{:?}", final_status),
                        "instance_id": instance_id,
                        "is_terminal": matches!(final_status, ExecutionStatus::Completed | ExecutionStatus::Failed)
                    })
                );
            }
        }

        WorkflowCommands::Status { instance_id } => {
            let manager = CollectionsManager::new(
                &ctx.storage,
                &ctx.config.system_domain,
                &ctx.config.system_db,
            );

            match manager
                .get_document("workflow_instances", &instance_id)
                .await
            {
                Ok(Some(doc)) => {
                    let instance: WorkflowInstance = json::deserialize_from_value(doc).unwrap();
                    user_info!(
                        "INSTANCE_STATE_SYNC",
                        json_value!({
                            "status": format!("{:?}", instance.status),
                            "instance_handle": instance.handle
                        })
                    );

                    user_info!(
                        "INSTANCE_NODES_SNAPSHOT",
                        json_value!({
                            "nodes": instance.node_states,
                            "count": instance.node_states.len()
                        })
                    );

                    if let Some(last_log) = instance.logs.last() {
                        user_info!("INSTANCE_LAST_EVENT", json_value!({ "log": last_log }));
                    }
                }
                _ => user_error!(
                    "INSTANCE_NOT_FOUND",
                    json_value!({
                        "instance_id": instance_id,
                        "action": "lookup_failure",
                        "severity": "medium"
                    })
                ),
            }
        }

        WorkflowCommands::SetSensor { value } => {
            let manager = CollectionsManager::new(&ctx.storage, &ctx.active_domain, &ctx.active_db);

            let sensor_doc = json_value!({
                "_id": "vibration_z",
                "value": value,
                "updatedAt": UtcClock::now().to_rfc3339()
            });

            manager.upsert_document("digital_twin", sensor_doc).await?;

            user_success!(
                "SENSOR_UPDATED",
                json_value!({
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
#[cfg(test)]
mod tests {
    use super::*;
    use raise::utils::context::SessionManager;

    #[cfg(test)]
    use raise::utils::testing::GlobalDbSandbox;

    #[async_test]
    #[serial_test::serial]
    async fn test_cli_set_sensor_writes_to_db() {
        let sandbox = GlobalDbSandbox::new().await;

        raise::json_db::jsonld::VocabularyRegistry::init_mock_for_tests();

        let manager = CollectionsManager::new(
            &sandbox.db,
            &sandbox.config.system_domain,
            &sandbox.config.system_db,
        );
        manager
            .create_collection(
                "digital_twin",
                "db://_system/_system/schemas/v1/db/generic.schema.json",
            )
            .await
            .unwrap();

        let ctx = CliContext::mock(
            AppConfig::get(),
            SessionManager::new(sandbox.db.clone()),
            sandbox.db.clone(),
        );

        let mut ctx = ctx;
        ctx.active_domain = sandbox.config.system_domain.clone();
        ctx.active_db = sandbox.config.system_db.clone();

        let args = WorkflowArgs {
            command: WorkflowCommands::SetSensor { value: 42.5 },
        };

        let result = handle(args, ctx).await;

        if let Err(e) = &result {
            panic!("❌ La commande SetSensor a échoué avec l'erreur : {:?}", e);
        }

        let manager = CollectionsManager::new(
            &sandbox.db,
            &sandbox.config.system_domain,
            &sandbox.config.system_db,
        );

        let doc = manager
            .get_document("digital_twin", "vibration_z")
            .await
            .expect("Erreur DB")
            .expect("Le capteur vibration_z n'a pas été trouvé en base");

        assert_eq!(doc["value"], 42.5, "La valeur en base ne correspond pas");
    }

    #[async_test]
    #[serial_test::serial]
    async fn test_cli_submit_mandate_persists() {
        let sandbox = GlobalDbSandbox::new().await;

        raise::json_db::jsonld::VocabularyRegistry::init_mock_for_tests();

        let mandate_path = sandbox.domain_root.join("test_mandate.json");
        let manager = CollectionsManager::new(
            &sandbox.db,
            &sandbox.config.system_domain,
            &sandbox.config.system_db,
        );
        manager
            .create_collection(
                "mandates",
                "db://_system/_system/schemas/v1/db/generic.schema.json",
            )
            .await
            .unwrap();

        let ctx = CliContext::mock(
            AppConfig::get(),
            SessionManager::new(sandbox.db.clone()),
            sandbox.db.clone(),
        );

        let mut ctx = ctx;
        ctx.active_domain = sandbox.config.system_domain.clone();
        ctx.active_db = sandbox.config.system_db.clone();

        let valid_mandate = json_value!({
            "handle": "mandate_cli_test_123",
            "name": { "fr": "Mandat de Test" },
            "meta": { "mandator_id": "00000000-0000-0000-0000-000000000000", "version": "1.0.0", "status": "ACTIVE" },
            "governance": { "strategy": "SAFETY_FIRST", "condorcetWeights": { "sec": 1.0 } },
            "hardLogic": { "vetos": [] },
            "observability": { "heartbeatMs": 100 }
        });
        fs::write_async(&mandate_path, valid_mandate.to_string())
            .await
            .unwrap();

        let args = WorkflowArgs {
            command: WorkflowCommands::SubmitMandate {
                path: mandate_path.to_string_lossy().to_string(),
            },
        };

        let result = handle(args, ctx).await;

        if let Err(e) = &result {
            panic!(
                "❌ La commande SubmitMandate a échoué avec l'erreur : {:?}",
                e
            );
        }

        let manager = CollectionsManager::new(
            &sandbox.db,
            &sandbox.config.system_domain,
            &sandbox.config.system_db,
        );

        let doc = manager
            .get_document("mandates", "mandate_cli_test_123")
            .await
            .unwrap()
            .expect("Le mandat n'a pas été sauvegardé dans la collection 'mandates' !");

        // 🎯 FIX : On vérifie bien le `mandator_id` en UUID et non l'ancien `author`
        assert_eq!(
            doc["meta"]["mandator_id"], "00000000-0000-0000-0000-000000000000",
            "Le mandat trouvé ne correspond pas"
        );
    }

    #[async_test]
    #[serial_test::serial]
    async fn test_cli_submit_mandate_invalid_path_fails_safely() {
        let sandbox = GlobalDbSandbox::new().await;

        let ctx = CliContext::mock(
            AppConfig::get(),
            SessionManager::new(sandbox.db.clone()),
            sandbox.db.clone(),
        );

        let fake_path = "path/to/nothing.yaml";
        let args = WorkflowArgs {
            command: WorkflowCommands::SubmitMandate {
                path: fake_path.to_string(),
            },
        };

        let result = super::handle(args, ctx).await;

        assert!(
            result.is_err(),
            "Le CLI ne doit pas paniquer mais retourner une Err pour un fichier manquant"
        );

        let err_msg = result.unwrap_err().to_string();

        assert!(
            err_msg.contains("FS_MANDATE_NOT_FOUND"),
            "L'erreur attendue était FS_MANDATE_NOT_FOUND. Reçu : {}",
            err_msg
        );
    }
}
