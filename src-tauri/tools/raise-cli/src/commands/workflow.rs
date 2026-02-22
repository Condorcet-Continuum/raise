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
async fn init_cli_engine() -> Result<(StorageEngine, WorkflowScheduler, String, String)> {
    let config = AppConfig::get();
    let db_root = config
        .get_path("PATH_RAISE_DOMAIN")
        .unwrap_or_else(|| std::path::PathBuf::from("./_system"));

    let storage = StorageEngine::new(JsonDbConfig::new(db_root));
    let domain = config.system_domain.clone();
    let db = config.system_db.clone();

    // Initialisation du moteur (Léger pour le CLI)
    let orch = AiOrchestrator::new(ProjectModel::default(), None)
        .await
        .map_err(|e| AppError::from(format!("Erreur Init IA: {}", e)))?;
    let pm = Arc::new(PluginManager::new(&storage, None));

    let executor = WorkflowExecutor::new(Arc::new(AsyncMutex::new(orch)), pm);
    let scheduler = WorkflowScheduler::new(executor);

    Ok((storage, scheduler, domain, db))
}

// --- POINT D'ENTRÉE PRINCIPAL ---
pub async fn handle(args: WorkflowArgs) -> Result<()> {
    match args.command {
        WorkflowCommands::SubmitMandate { path } => {
            user_info!("MANDATE", "Chargement du mandat depuis : {}", path);
            let path_ref = io::Path::new(&path);

            if !io::exists(path_ref).await {
                user_error!("FS_ERROR", "Fichier de mandat introuvable : {}", path);
                return Ok(());
            }

            let content = tokio::fs::read_to_string(path_ref)
                .await
                .map_err(|e| AppError::from(format!("Impossible de lire le fichier : {}", e)))?;

            let mandate: Mandate = serde_json::from_str(&content)
                .map_err(|e| AppError::from(format!("Validation JSON échouée : {}", e)))?;

            // 1. Compilation pour vérifier la validité
            let definition = WorkflowCompiler::compile(&mandate);

            // 2. Connexion à la base de données pour persister le Mandat
            let (storage, _sched, domain, db) = init_cli_engine().await?;
            let manager = CollectionsManager::new(&storage, &domain, &db);

            let json_mandate = serde_json::to_value(&mandate).unwrap();

            // Persistance pour que le serveur Tauri puisse le trouver
            manager
                .insert_raw("mandates", &json_mandate)
                .await
                .map_err(|e| {
                    AppError::Database(format!("Échec de sauvegarde du mandat : {}", e))
                })?;

            user_success!(
                "COMPILE_OK",
                "Mandat '{}' v{} compilé et persisté avec succès. ID Graphe : {}",
                mandate.meta.author,
                mandate.meta.version,
                definition.id
            );
        }

        WorkflowCommands::Start { workflow_id } => {
            user_info!(
                "ENGINE",
                "Démarrage du moteur pour le workflow : {}",
                workflow_id
            );
            let (storage, mut scheduler, domain, db) = init_cli_engine().await?;
            let manager = CollectionsManager::new(&storage, &domain, &db);

            // Hack CLI: On "charge" la définition dans le scheduler courant en feignant le mandate_id
            let mandate_id = workflow_id.replace("wf_", "");
            scheduler.load_mission(&mandate_id, &manager).await?;

            let instance = scheduler.create_instance(&workflow_id, &manager).await?;
            user_success!("START", "Instance créée avec l'ID : {}", instance.id);

            let final_status = scheduler
                .execute_instance_loop(&instance.id, &manager)
                .await?;

            match final_status {
                ExecutionStatus::Completed => {
                    user_success!("DONE", "Workflow terminé avec succès !")
                }
                ExecutionStatus::Paused => {
                    user_info!("HITL", "Le workflow est en pause (Validation requise).")
                }
                ExecutionStatus::Failed => user_error!("FAIL", "Le workflow a échoué."),
                _ => user_info!("STATUS", "Statut final : {:?}", final_status),
            }
        }

        WorkflowCommands::Resume {
            instance_id,
            node_id,
            approved,
        } => {
            user_info!(
                "HITL",
                "Reprise de l'instance {} (Nœud: {}, Décision: {})",
                instance_id,
                node_id,
                approved
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
                user_info!("STATUS", "Nouveau statut : {:?}", final_status);
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
                    user_info!("STATE", "Statut: {:?}", instance.status);
                    user_info!("INFO", "Nœuds courants: {:?}", instance.node_states);

                    if let Some(last_log) = instance.logs.last() {
                        user_info!("LAST_LOG", "{}", last_log);
                    }
                }
                _ => user_error!(
                    "NOT_FOUND",
                    "Aucune instance trouvée pour l'ID {}",
                    instance_id
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
            manager
                .insert_raw("digital_twin", &sensor_doc)
                .await
                .map_err(|e| AppError::Database(format!("Erreur d'écriture capteur: {}", e)))?;

            user_success!(
                "SENSOR_UPDATED",
                "Jumeau Numérique (vibration_z) mis à jour en base : {:.2}",
                value
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
    async fn test_cli_submit_mandate_invalid_path_fails_safely() {
        let args = WorkflowArgs {
            command: WorkflowCommands::SubmitMandate {
                path: "/chemin/vers/un/fichier/inexistant.json".to_string(),
            },
        };

        let result = handle(args).await;
        assert!(
            result.is_ok(),
            "Le CLI ne doit pas crasher sur un fichier manquant"
        );
    }
}
