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

// 🎯 Import du contexte global CLI
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
    SubmitMandate { path: String },
    /// Compile une mission métier en un graphe d'exécution
    CompileMission { mission_id: String },
    /// Met à jour une valeur de capteur (Jumeau Numérique local)
    SetSensor { value: f64 },
    /// Démarre une nouvelle instance à partir d'un graphe compilé
    Start {
        mission_id: String,
        workflow_id: String,
    },
    /// Reprend un workflow en attente de validation (HITL)
    Resume {
        instance_id: String,
        node_id: String,
        #[arg(short, long)]
        approved: bool,
    },
    /// Affiche le statut détaillé d'une instance depuis la base de données
    Status { instance_id: String },
}

// --- HELPER D'INITIALISATION DU MOTEUR ---
// 🎯 Résilience : On utilise le contexte global et les points de montage
async fn init_cli_engine(ctx: &CliContext) -> RaiseResult<WorkflowScheduler> {
    let manager = CollectionsManager::new(&ctx.storage, &ctx.active_domain, &ctx.active_db);

    // Initialisation de l'Orchestrateur avec gestion d'erreur stricte
    let orch =
        match AiOrchestrator::new(ProjectModel::default(), &manager, ctx.storage.clone()).await {
            Ok(instance) => instance,
            Err(e) => raise_error!(
                "ERR_AI_ORCHESTRATOR_INIT",
                error = e,
                context = json_value!({
                    "action": "startup_ai_engine",
                    "hint": "Vérifiez la VRAM et les points de montage système."
                })
            ),
        };

    let pm = SharedRef::new(PluginManager::new(&ctx.storage, None));
    let executor = WorkflowExecutor::new(SharedRef::new(AsyncMutex::new(orch)), pm);

    Ok(WorkflowScheduler::new(executor))
}

// --- POINT D'ENTRÉE PRINCIPAL ---
pub async fn handle(args: WorkflowArgs, ctx: CliContext) -> RaiseResult<()> {
    // 🎯 Heartbeat de session
    match ctx.session_mgr.touch().await {
        Ok(_) => user_debug!("SESSION_TOUCHED"),
        Err(e) => user_error!(
            "ERR_SESSION_HEARTBEAT",
            json_value!({"error": e.to_string()})
        ),
    }

    match args.command {
        WorkflowCommands::SubmitMandate { path } => {
            user_info!("MANDATE_LOAD_START", json_value!({ "path": path }));
            let path_ref = Path::new(&path);

            if !fs::exists_async(path_ref).await {
                raise_error!(
                    "FS_MANDATE_NOT_FOUND",
                    error = "File missing",
                    context = json_value!({"path": path})
                );
            }

            let content = match fs::read_to_string_async(path_ref).await {
                Ok(c) => c,
                Err(e) => raise_error!(
                    "ERR_FS_READ",
                    error = e,
                    context = json_value!({"path": path})
                ),
            };

            let mandate: Mandate = match json::deserialize_from_str(&content) {
                Ok(m) => m,
                Err(e) => raise_error!(
                    "ERR_JSON_PARSE",
                    error = e,
                    context = json_value!({"action": "parse_mandate"})
                ),
            };

            let manager = CollectionsManager::new(&ctx.storage, &ctx.active_domain, &ctx.active_db);
            let json_mandate = json::serialize_to_value(&mandate).expect("Serialization fail");

            manager.upsert_document("mandates", json_mandate).await?;

            user_success!(
                "MANDATE_IMPORT_SUCCESS",
                json_value!({
                    "mandator_id": mandate.meta.mandator_id,
                    "domain": ctx.active_domain
                })
            );
        }

        WorkflowCommands::CompileMission { mission_id } => {
            let manager = CollectionsManager::new(&ctx.storage, &ctx.active_domain, &ctx.active_db);
            let definition = WorkflowCompiler::compile(&manager, &mission_id).await?;

            user_success!(
                "MISSION_COMPILE_SUCCESS",
                json_value!({
                    "mission_id": mission_id,
                    "graph_handle": definition.handle
                })
            );
        }

        WorkflowCommands::Start {
            mission_id,
            workflow_id,
        } => {
            let mut scheduler = init_cli_engine(&ctx).await?;
            let manager = CollectionsManager::new(&ctx.storage, &ctx.active_domain, &ctx.active_db);

            scheduler.load_mission(&mission_id, &manager).await?;
            let instance = scheduler
                .create_instance(&mission_id, &workflow_id, &manager)
                .await?;

            user_success!(
                "INSTANCE_INITIALIZED",
                json_value!({"handle": instance.handle})
            );

            let final_status = scheduler
                .execute_instance_loop(&instance.handle, &manager)
                .await?;

            match final_status {
                ExecutionStatus::Completed => user_success!("WORKFLOW_COMPLETED"),
                ExecutionStatus::Paused => user_info!(
                    "WORKFLOW_PAUSED_HITL",
                    json_value!({"handle": instance.handle})
                ),
                _ => user_error!(
                    "WORKFLOW_TERMINATED_ABNORMALLY",
                    json_value!({"status": format!("{:?}", final_status)})
                ),
            }
        }

        WorkflowCommands::Resume {
            instance_id,
            node_id,
            approved,
        } => {
            let mut scheduler = init_cli_engine(&ctx).await?;
            let manager = CollectionsManager::new(&ctx.storage, &ctx.active_domain, &ctx.active_db);

            let doc = match manager
                .get_document("workflow_instances", &instance_id)
                .await?
            {
                Some(d) => d,
                None => raise_error!(
                    "INSTANCE_NOT_FOUND",
                    context = json_value!({"id": instance_id})
                ),
            };

            let instance: WorkflowInstance =
                json::deserialize_from_value(doc).expect("Deserialization fail");
            scheduler
                .load_mission(&instance.mission_id, &manager)
                .await?;
            scheduler
                .resume_node(&instance_id, &node_id, approved, &manager)
                .await?;

            let final_status = scheduler
                .execute_instance_loop(&instance_id, &manager)
                .await?;
            user_info!(
                "WORKFLOW_RESUMED",
                json_value!({"final_status": format!("{:?}", final_status)})
            );
        }

        WorkflowCommands::Status { instance_id } => {
            // Résilience : Utilisation des points de montage système pour le monitoring
            let manager = CollectionsManager::new(
                &ctx.storage,
                &ctx.config.mount_points.system.domain,
                &ctx.config.mount_points.system.db,
            );

            match manager
                .get_document("workflow_instances", &instance_id)
                .await?
            {
                Some(doc) => {
                    let instance: WorkflowInstance =
                        json::deserialize_from_value(doc).expect("Deserialization fail");
                    user_info!(
                        "INSTANCE_SYNC",
                        json_value!({
                            "status": format!("{:?}", instance.status),
                            "nodes": instance.node_states.len()
                        })
                    );
                }
                None => user_error!("INSTANCE_NOT_FOUND"),
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
            user_success!("SENSOR_UPDATED", json_value!({"value": value}));
        }
    }
    Ok(())
}

// =========================================================================
// TESTS UNITAIRES ET RÉSILIENCE
// =========================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use raise::utils::context::SessionManager;
    use raise::utils::testing::{AgentDbSandbox, DbSandbox};

    #[async_test]
    #[serial_test::serial]
    async fn test_cli_set_sensor_writes_to_db() {
        let sandbox = AgentDbSandbox::new().await;
        raise::json_db::jsonld::VocabularyRegistry::init_mock_for_tests();

        let manager = CollectionsManager::new(
            &sandbox.db,
            &sandbox.config.mount_points.system.domain,
            &sandbox.config.mount_points.system.db,
        );
        DbSandbox::mock_db(&manager).await.expect("Init index fail");

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
        ctx.active_domain = sandbox.config.mount_points.system.domain.clone();
        ctx.active_db = sandbox.config.mount_points.system.db.clone();

        handle(
            WorkflowArgs {
                command: WorkflowCommands::SetSensor { value: 42.5 },
            },
            ctx,
        )
        .await
        .unwrap();

        let doc = manager
            .get_document("digital_twin", "vibration_z")
            .await
            .unwrap()
            .expect("Doc missing");
        assert_eq!(doc["value"], 42.5);
    }

    #[async_test]
    #[serial_test::serial]
    async fn test_cli_submit_mandate_persists() {
        let sandbox = AgentDbSandbox::new().await;
        raise::json_db::jsonld::VocabularyRegistry::init_mock_for_tests();

        let mandate_path = sandbox.domain_root.join("test_mandate.json");
        let manager = CollectionsManager::new(
            &sandbox.db,
            &sandbox.config.mount_points.system.domain,
            &sandbox.config.mount_points.system.db,
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
        ctx.active_domain = sandbox.config.mount_points.system.domain.clone();
        ctx.active_db = sandbox.config.mount_points.system.db.clone();

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

        handle(
            WorkflowArgs {
                command: WorkflowCommands::SubmitMandate {
                    path: mandate_path.to_string_lossy().to_string(),
                },
            },
            ctx,
        )
        .await
        .unwrap();

        let doc = manager
            .get_document("mandates", "mandate_cli_test_123")
            .await
            .unwrap()
            .expect("Mandate missing");
        assert_eq!(
            doc["meta"]["mandator_id"],
            "00000000-0000-0000-0000-000000000000"
        );
    }

    /// 🎯 NOUVEAU TEST : Résilience de la résolution des Mount Points en mode Workflow
    #[async_test]
    async fn test_workflow_mount_point_integrity() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await;
        assert!(
            !sandbox.config.mount_points.system.domain.is_empty(),
            "Partition système non résolue"
        );
        assert!(
            !sandbox.config.mount_points.system.db.is_empty(),
            "Base système non résolue"
        );
        Ok(())
    }
}
