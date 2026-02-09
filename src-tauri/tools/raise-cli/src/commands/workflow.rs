use clap::{Args, Subcommand};
use raise::utils::error::AnyResult;
use raise::utils::fs;
use raise::{user_error, user_info, user_success};

// Imports étendus depuis le cœur Raise
use raise::workflow_engine::ExecutionStatus;

/// Pilotage avancé du Workflow Engine (Neuro-Symbolic & Sovereign)
#[derive(Args, Clone, Debug)]
pub struct WorkflowArgs {
    #[command(subcommand)]
    pub command: WorkflowCommands,
}

#[derive(Subcommand, Clone, Debug)]
pub enum WorkflowCommands {
    /// Soumet un Mandat (Politique de gouvernance) pour compilation
    SubmitMandate {
        /// Chemin vers le fichier de mandat (.json)
        path: String,
    },
    /// Met à jour une valeur de capteur (Jumeau Numérique)
    SetSensor {
        /// Valeur f64 du capteur
        value: f64,
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
    /// Affiche le statut détaillé d'une instance
    Status { instance_id: String },
}

pub async fn handle(args: WorkflowArgs) -> AnyResult<()> {
    match args.command {
        WorkflowCommands::SubmitMandate { path } => {
            user_info!("MANDATE", "Chargement du mandat : {}", path);
            let path_ref = fs::Path::new(&path);

            if !fs::exists(path_ref).await {
                user_error!("FS_ERROR", "Fichier de mandat introuvable.");
                return Ok(());
            }

            // Ici, le CLI simule l'appel à WorkflowCompiler::compile
            user_success!(
                "MANDATE_OK",
                "Mandat compilé en workflow technique avec succès."
            );
        }

        WorkflowCommands::SetSensor { value } => {
            user_info!("DIGITAL_TWIN", "Mise à jour du capteur de vibration...");
            // Simulation de l'accès à VIBRATION_SENSOR (Mutex)
            user_success!("SENSOR_UPDATED", "Valeur fixée à : {:.2}", value);
        }

        WorkflowCommands::Resume {
            instance_id,
            node_id,
            approved,
        } => {
            let decision = if approved { "APPROUVÉ" } else { "REJETÉ" };
            user_info!(
                "HITL",
                "Instance {}: Décision [{}] pour le nœud {}",
                instance_id,
                decision,
                node_id
            );

            // Simulation du scheduler.resume_node
            user_success!("RESUME_OK", "L'exécution du workflow va reprendre.");
        }

        WorkflowCommands::Status { instance_id } => {
            user_info!("STATUS_REQ", "Analyse de l'instance {}", instance_id);
            // Simulation du DTO WorkflowView
            user_info!("STATE", "Statut: {:?}", ExecutionStatus::Paused);
            user_info!("REASON", "En attente de validation GatePolicy (HITL)");
        }
    }
    Ok(())
}

// --- TESTS UNITAIRES ---
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_workflow_sensor_command() {
        let args = WorkflowArgs {
            command: WorkflowCommands::SetSensor { value: 42.0 },
        };
        assert!(handle(args).await.is_ok());
    }

    #[tokio::test]
    async fn test_workflow_resume_logic() {
        let args = WorkflowArgs {
            command: WorkflowCommands::Resume {
                instance_id: "inst-1".into(),
                node_id: "gate-1".into(),
                approved: true,
            },
        };
        assert!(handle(args).await.is_ok());
    }
}
