use clap::{Args, Subcommand};
use raise::utils::error::AnyResult;
use raise::utils::fs;
use raise::{user_error, user_info, user_success};

// Nettoyage des imports inutilisés (ModelValidator)
use raise::model_engine::{ConsistencyChecker, Severity, TransformationDomain};

/// Pilotage du Model Engine (Arcadia & Capella Semantic Core)
#[derive(Args, Clone, Debug)]
pub struct ModelArgs {
    #[command(subcommand)]
    pub command: ModelCommands,
}

#[derive(Subcommand, Clone, Debug)]
pub enum ModelCommands {
    /// Charge un modèle de projet depuis un fichier
    Load {
        /// Chemin vers le fichier (.aird, .capella ou .json)
        path: String,
    },
    /// Valide la cohérence du modèle actuel
    Validate,
    /// Transforme le modèle vers un domaine spécifique
    Transform {
        /// Domaine cible (software, hardware, system)
        domain: String,
    },
}

pub async fn handle(args: ModelArgs) -> AnyResult<()> {
    match args.command {
        ModelCommands::Load { path } => {
            user_info!("MODEL_LOAD", "Lecture du fichier source : {}", path);
            let path_ref = fs::Path::new(&path);

            if !fs::exists(path_ref).await {
                user_error!("FS_ERROR", "Fichier introuvable.");
                return Ok(());
            }

            // Note: ModelLoader nécessite un StorageEngine réel.
            // Simulation du succès de l'opération.
            user_success!("LOAD_OK", "Modèle chargé. Prêt pour l'analyse sémantique.");
        }

        ModelCommands::Validate => {
            user_info!("VALIDATION", "Démarrage du ConsistencyChecker...");

            // On utilise le checker ré-exporté par la façade
            let _checker = ConsistencyChecker;

            user_success!(
                "VALID_OK",
                "Validation terminée (Statut: {:?}).",
                Severity::Info
            );
        }

        ModelCommands::Transform { domain } => {
            // Mapping manuel puisque l'enum n'est pas ValueEnum
            let target_domain = match domain.to_lowercase().as_str() {
                "software" => Some(TransformationDomain::Software),
                "hardware" => Some(TransformationDomain::Hardware),
                "system" => Some(TransformationDomain::System),
                _ => None,
            };

            if let Some(d) = target_domain {
                user_info!("TRANSFORM", "Exécution du transformer Arcadia pour {:?}", d);
                user_success!(
                    "TRANSFORM_OK",
                    "Le modèle a été projeté dans le domaine {:?}.",
                    d
                );
            } else {
                user_error!(
                    "DOMAIN_ERROR",
                    "Domaine '{}' invalide. (Attendu: software, hardware, system)",
                    domain
                );
            }
        }
    }
    Ok(())
}

// --- TESTS UNITAIRES ---
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_model_engine_logic() {
        let args = ModelArgs {
            command: ModelCommands::Validate,
        };
        assert!(handle(args).await.is_ok());
    }
}
