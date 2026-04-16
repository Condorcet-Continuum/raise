// FICHIER : src-tauri/tools/raise-cli/src/commands/workflow.rs

use clap::{Args, Subcommand};
use raise::utils::prelude::*; // 🎯 Façade Unique RAISE

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
    /// Affiche le statut détaillé d'une instance
    Status { instance_id: String },
}

// --- HELPER D'INITIALISATION DU MOTEUR ---
async fn init_cli_engine(ctx: &CliContext) -> RaiseResult<WorkflowScheduler> {
    let manager = CollectionsManager::new(&ctx.storage, &ctx.active_domain, &ctx.active_db);

    let orch =
        match AiOrchestrator::new(ProjectModel::default(), &manager, ctx.storage.clone()).await {
            Ok(instance) => instance,
            Err(e) => raise_error!(
                "ERR_AI_ORCHESTRATOR_INIT",
                error = e,
                context = json_value!({ "hint": "Vérifiez la VRAM et les points de montage." })
            ),
        };

    let pm = SharedRef::new(PluginManager::new(&ctx.storage, None));
    let executor = WorkflowExecutor::new(SharedRef::new(AsyncMutex::new(orch)), pm);

    Ok(WorkflowScheduler::new(executor))
}

pub async fn handle(args: WorkflowArgs, ctx: CliContext) -> RaiseResult<()> {
    // 🎯 Heartbeat de session
    if let Err(e) = ctx.session_mgr.touch().await {
        user_error!(
            "ERR_SESSION_HEARTBEAT",
            json_value!({"error": e.to_string()})
        );
    }

    match args.command {
        WorkflowCommands::SubmitMandate { path } => {
            // 🎯 FIX : Utilisation d'une référence &path pour éviter le move
            user_info!("MANDATE_LOAD_START", json_value!({ "path": &path }));
            let path_ref = Path::new(&path);

            if !fs::exists_async(path_ref).await {
                raise_error!(
                    "FS_MANDATE_NOT_FOUND",
                    error = "Fichier manquant",
                    context = json_value!({"path": path})
                );
            }

            let content = fs::read_to_string_async(path_ref).await?;
            let mandate: Mandate = json::deserialize_from_str(&content)
                .map_err(|e| build_error!("ERR_JSON_PARSE", error = e))?;

            let manager = CollectionsManager::new(&ctx.storage, &ctx.active_domain, &ctx.active_db);
            let json_mandate = json::serialize_to_value(&mandate)?;

            manager.upsert_document("mandates", json_mandate).await?;

            user_success!(
                "MANDATE_IMPORT_SUCCESS",
                json_value!({ "id": mandate.meta.mandator_id })
            );
        }

        WorkflowCommands::CompileMission { mission_id } => {
            let manager = CollectionsManager::new(&ctx.storage, &ctx.active_domain, &ctx.active_db);
            let definition = WorkflowCompiler::compile(&manager, &mission_id).await?;

            user_success!(
                "MISSION_COMPILE_SUCCESS",
                json_value!({ "handle": definition.handle })
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
                json_value!({"handle": &instance.handle})
            );

            match scheduler
                .execute_instance_loop(&instance.handle, &manager)
                .await?
            {
                ExecutionStatus::Completed => user_success!("WORKFLOW_COMPLETED"),
                ExecutionStatus::Paused => user_info!(
                    "WORKFLOW_PAUSED_HITL",
                    json_value!({"handle": instance.handle})
                ),
                status => user_error!(
                    "WORKFLOW_TERMINATED",
                    json_value!({"status": format!("{:?}", status)})
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

            let doc = manager
                .get_document("workflow_instances", &instance_id)
                .await?
                .ok_or_else(|| build_error!("INSTANCE_NOT_FOUND", error = instance_id.clone()))?;

            let instance: WorkflowInstance = json::deserialize_from_value(doc)?;

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
                json_value!({"status": format!("{:?}", final_status)})
            );
        }

        WorkflowCommands::Status { instance_id } => {
            let manager = CollectionsManager::new(&ctx.storage, &ctx.active_domain, &ctx.active_db);
            let doc = manager
                .get_document("workflow_instances", &instance_id)
                .await?
                .ok_or_else(|| build_error!("INSTANCE_NOT_FOUND"))?;

            let instance: WorkflowInstance = json::deserialize_from_value(doc)?;
            user_info!(
                "INSTANCE_SYNC",
                json_value!({ "status": format!("{:?}", instance.status) })
            );
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
// TESTS UNITAIRES (Conformité « Zéro Dette »)
// =========================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use raise::utils::testing::{AgentDbSandbox, DbSandbox};

    #[async_test]
    #[serial_test::serial] // 🎯 FIX : Empêche les conflits de session et de VRAM
    async fn test_cli_set_sensor_writes_to_db() -> RaiseResult<()> {
        raise::json_db::jsonld::VocabularyRegistry::init_mock_for_tests();
        let sandbox = AgentDbSandbox::new().await;

        let config = AppConfig::get();
        let storage = sandbox.db.clone();
        let session_mgr = crate::context::SessionManager::new(storage.clone());

        // 1. Initialisation du contexte CLI mocké
        let ctx = CliContext::mock(config, session_mgr, storage);

        // 2. 🎯 FIX : On crée la collection dans le domaine ACTIF du contexte (domaine métier)
        // C'est ici que résidait l'erreur : on pointait sur la partition système au lieu de mock_domain
        let manager = CollectionsManager::new(&ctx.storage, &ctx.active_domain, &ctx.active_db);

        // Initialisation de la base de données de travail
        DbSandbox::mock_db(&manager).await?;

        manager
            .create_collection(
                "digital_twin",
                "db://_system/_system/schemas/v1/db/generic.schema.json",
            )
            .await?;

        // 3. Exécution de la commande
        handle(
            WorkflowArgs {
                command: WorkflowCommands::SetSensor { value: 42.5 },
            },
            ctx.clone(),
        )
        .await?;

        // 4. Vérification
        let doc = manager
            .get_document("digital_twin", "vibration_z")
            .await?
            .ok_or_else(|| build_error!("ERR_TEST", error = "Document introuvable"))?;

        assert_eq!(doc["value"], 42.5);
        Ok(())
    }

    #[async_test]
    #[serial_test::serial]
    async fn test_workflow_mount_point_integrity() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await;
        assert!(!sandbox.config.mount_points.system.domain.is_empty());
        Ok(())
    }
}
