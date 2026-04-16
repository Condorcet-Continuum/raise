// FICHIER : src-tauri/tools/raise-cli/src/commands/validator.rs

use clap::Args;
use raise::utils::prelude::*; // 🎯 Façade Unique RAISE

use raise::json_db::schema::{SchemaRegistry, SchemaValidator};
use raise::json_db::storage::JsonDbConfig;

// 🎯 Import du contexte global CLI
use crate::CliContext;

#[derive(Args, Debug, Clone)]
pub struct ValidatorArgs {
    /// Chemin relatif du fichier de données (ex: data/dapps/tva-manager.json)
    #[arg(short, long)]
    pub data: String,

    /// URI du schéma cible (ex: dapps/dapp.schema.json)
    #[arg(short, long)]
    pub schema: String,
}

pub async fn handle(args: ValidatorArgs, ctx: CliContext) -> RaiseResult<()> {
    // 🎯 Heartbeat de session : On traite l'erreur pour la traçabilité sémantique
    if let Err(e) = ctx.session_mgr.touch().await {
        user_error!(
            "ERR_SESSION_HEARTBEAT",
            json_value!({"error": e.to_string()})
        );
    }

    let app_config = ctx.config;

    // Résolution du chemin racine du domaine
    let domain_root = app_config.get_path("PATH_RAISE_DOMAIN").ok_or_else(|| {
        build_error!(
            "CLI_MISSING_DOMAIN_PATH",
            error = "PATH_RAISE_DOMAIN introuvable."
        )
    })?;

    let dataset_root = app_config
        .get_path("PATH_RAISE_DATASET")
        .unwrap_or_else(|| domain_root.join("dataset"));

    // Vérification de l'existence des dossiers critiques
    if !dataset_root.exists() || !domain_root.exists() {
        raise_error!(
            "FS_DIRECTORY_MISSING",
            error = "Infrastucture de données incomplète.",
            context = json_value!({ "domain": domain_root, "dataset": dataset_root })
        );
    }

    let db_root = if domain_root.join("un2").exists() {
        domain_root.join("un2")
    } else {
        domain_root
    };
    let cfg = JsonDbConfig::new(db_root);

    let space = "_system";
    let db_name = "schemas";

    let registry = SchemaRegistry::from_db(&cfg, space, db_name)
        .await
        .map_err(|e| build_error!("SCHEMA_REGISTRY_LOAD_FAIL", error = e))?;

    // Chargement asynchrone du document
    let data_full_path = dataset_root.join(&args.data);
    let mut doc: JsonValue = fs::read_json_async(&data_full_path).await?;

    let full_uri = registry.uri(&args.schema);

    // 🎯 FIX : Utilisation d'une référence (&full_uri) pour éviter le move dans la macro
    user_info!(
        "VALIDATOR_START",
        json_value!({ "uri": &full_uri, "domain": ctx.active_domain })
    );

    let validator = SchemaValidator::compile_with_registry(&full_uri, &registry)
        .map_err(|e| build_error!("SCHEMA_COMPILATION_FAILED", error = e))?;

    match validator.compute_then_validate(&mut doc) {
        Ok(_) => {
            user_success!("VALIDATOR_SUCCESS", json_value!({"status": "passed"}));
            if let Some(id) = doc.get("_id") {
                user_info!("VALIDATOR_ID", json_value!({ "_id": id }));
            }
            Ok(())
        }
        Err(e) => {
            // 🎯 Divergence : La macro renvoie Err(AppError), pas de return requis
            raise_error!(
                "VALIDATOR_FAILURE",
                error = e,
                context = json_value!({ "schema_uri": full_uri })
            )
        }
    }
}

// =========================================================================
// TESTS UNITAIRES (Conformité « Zéro Dette »)
// =========================================================================

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
    #[serial_test::serial] // 🎯 FIX : Isolation pour la CI
    fn test_validator_parsing_logic() -> RaiseResult<()> {
        let args = vec!["test", "--data", "f.json", "--schema", "s.json"];
        let cli =
            TestCli::try_parse_from(args).map_err(|e| build_error!("ERR_PARSE", error = e))?;
        assert_eq!(cli.args.data, "f.json");
        Ok(())
    }

    #[test]
    #[serial_test::serial]
    fn test_path_logic_robustness() {
        let base = PathBuf::from("/tmp/raise");
        let full = base.join("data/test.json");
        assert!(full.to_string_lossy().contains("data/test.json"));
    }
}
