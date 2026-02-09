use clap::Args;

// Imports internes RAISE
// NOUVEAU : On utilise nos utilitaires sécurisés (env, fs, json)
use raise::utils::env;
use raise::utils::fs::{read_json, PathBuf};
use raise::utils::json::Value;
// NOUVEAU : On utilise notre gestion d'erreur centralisée
use raise::utils::error::{AnyResult, Context};

use raise::json_db::schema::{SchemaRegistry, SchemaValidator};
use raise::json_db::storage::JsonDbConfig;
use raise::utils::config::AppConfig;
use raise::{user_error, user_info, user_success};

#[derive(Args, Debug, Clone)]
pub struct ValidatorArgs {
    /// Chemin relatif du fichier de données DANS le dataset (ex: data/dapps/tva-manager.json)
    #[arg(short, long)]
    pub data: String,

    /// URI du schéma cible dans le registre (ex: dapps/dapp.schema.json)
    #[arg(short, long)]
    pub schema: String,
}

pub async fn handle(args: ValidatorArgs) -> AnyResult<()> {
    // 1. RÉCUPÉRATION DE LA CONFIGURATION
    let app_config = AppConfig::get();

    // Chemin DOMAIN (Pioché dans la config centralisée)
    let domain_root = app_config.database_root.clone();

    // Chemin DATASET (Reste spécifique à l'environnement local pour l'instant)
    // REFAC: env::get renvoie une AppError, mais on utilise le ? anyhow pour la convertir
    let dataset_path_str = env::get("PATH_RAISE_DATASET")?;
    let dataset_root = PathBuf::from(&dataset_path_str);

    // Vérification physique des dossiers racines
    if !dataset_root.exists() {
        return Err(raise::utils::error::anyhow!(
            "❌ Dossier Dataset introuvable : {:?}",
            dataset_root
        ));
    }
    if !domain_root.exists() {
        return Err(raise::utils::error::anyhow!(
            "❌ Dossier Domain introuvable : {:?}",
            domain_root
        ));
    }

    // 2. CONFIGURATION DE LA DB (Logique patrimoniale un2)
    let db_root = if domain_root.join("un2").exists() {
        domain_root.join("un2")
    } else {
        domain_root
    };

    let cfg = JsonDbConfig::new(db_root);

    // 3. CHARGEMENT DU REGISTRE
    let space = "_system";
    let db_name = "schemas";

    user_info!("VALIDATOR_LOADING_REGISTRY", "{}/{}", space, db_name);

    let registry = SchemaRegistry::from_db(&cfg, space, db_name)
        .await
        .context("Impossible de charger le registre des schémas depuis la DB")?;

    // 4. CHARGEMENT DE LA DONNÉE
    let data_full_path = dataset_root.join(&args.data);

    // REFACTOR : Lecture asynchrone, typée et sécurisée
    // Plus besoin de fs::read_to_string manuel ni de serde_json::from_str
    let mut doc: Value = read_json(&data_full_path).await?;

    // 5. VALIDATION
    let target_uri = &args.schema;
    let full_uri = registry.uri(target_uri);

    user_info!("VALIDATOR_START", "{}", full_uri);

    let validator = SchemaValidator::compile_with_registry(&full_uri, &registry)
        .context("Échec de la compilation du SchemaValidator")?;

    match validator.compute_then_validate(&mut doc) {
        Ok(_) => {
            user_success!("VALIDATOR_SUCCESS");
            if let Some(id) = doc.get("id") {
                user_info!("VALIDATOR_ID_GENERATED", "{}", id);
            }
            Ok(())
        }
        Err(e) => {
            user_error!("VALIDATOR_FAILURE");
            // On propage l'erreur pour qu'elle remonte proprement dans le main
            // On utilise raise::utils::error::anyhow! explicitement
            Err(raise::utils::error::anyhow!("{}", e))
        }
    }
}

// --- TESTS UNITAIRES (Patrimoine Conservé) ---
#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[derive(Parser)]
    struct TestCli {
        #[command(flatten)]
        args: ValidatorArgs,
    }

    #[test]
    fn test_validator_parsing() {
        // Vérifie que les arguments obligatoires sont bien capturés
        let args = vec!["test", "--data", "file.json", "--schema", "uri.json"];
        let cli = TestCli::parse_from(args);
        assert_eq!(cli.args.data, "file.json");
        assert_eq!(cli.args.schema, "uri.json");
    }

    #[test]
    fn test_validator_missing_args() {
        // Vérifie que le manque d'arguments provoque une erreur de parsing
        let args = vec!["test", "--data", "file.json"];
        let res = TestCli::try_parse_from(args);
        assert!(res.is_err());
    }

    #[test]
    fn test_path_logic_robustness() {
        // Teste la logique de construction de chemin sans accès disque
        let base = PathBuf::from("/tmp/raise");
        let sub = "data/test.json";
        let full = base.join(sub);
        assert!(full.to_string_lossy().contains("data/test.json"));
    }
}
