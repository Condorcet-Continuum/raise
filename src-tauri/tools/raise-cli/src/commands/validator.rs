// FICHIER : src-tauri/tools/raise-cli/src/commands/validator.rs

use clap::Args;

use raise::utils::prelude::*;

use raise::json_db::schema::{SchemaRegistry, SchemaValidator};
use raise::json_db::storage::JsonDbConfig;

// 🎯 Import du contexte global CLI
use crate::CliContext;

#[derive(Args, Debug, Clone)]
pub struct ValidatorArgs {
    /// Chemin relatif du fichier de données DANS le dataset (ex: data/dapps/tva-manager.json)
    #[arg(short, long)]
    pub data: String,

    /// URI du schéma cible dans le registre (ex: dapps/dapp.schema.json)
    #[arg(short, long)]
    pub schema: String,
}

// 🎯 La signature intègre le CliContext
pub async fn handle(args: ValidatorArgs, ctx: CliContext) -> RaiseResult<()> {
    // 🎯 Heartbeat automatique
    let _ = ctx.session_mgr.touch().await;

    // 1. RÉCUPÉRATION DE LA CONFIGURATION (via le contexte)
    let app_config = ctx.config;

    // 🎯 FIX CRITIQUE : Suppression du .expect() en production
    let domain_root = match app_config.get_path("PATH_RAISE_DOMAIN") {
        Some(path) => path,
        None => raise_error!(
            "CLI_MISSING_DOMAIN_PATH",
            error = "Le chemin PATH_RAISE_DOMAIN est introuvable !",
            context = json_value!({"required_for": "domain_root_resolution"})
        ),
    };

    // Chemin DATASET (Piloté par la config globale avec fallback sur le domaine)
    let dataset_root = app_config
        .get_path("PATH_RAISE_DATASET")
        .unwrap_or_else(|| domain_root.join("dataset"));

    // Vérification physique des dossiers racines
    if !dataset_root.exists() {
        raise_error!(
            "FS_DIRECTORY_MISSING",
            error = "Dataset root folder not found",
            context = json_value!({
                "path": dataset_root,
                "required_for": "data_persistence"
            })
        );
    }

    if !domain_root.exists() {
        raise_error!(
            "FS_DIRECTORY_MISSING",
            error = "Domain root folder not found",
            context = json_value!({
                "path": domain_root,
                "required_for": "domain_logic_isolation"
            })
        );
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

    user_info!(
        "VALIDATOR_LOADING_REGISTRY",
        json_value!({
            "space": space,
            "db": db_name,
            "action": "schema_fetch",
            "active_domain": ctx.active_domain,
            "active_user": ctx.active_user
        })
    );

    let registry = match SchemaRegistry::from_db(&cfg, space, db_name).await {
        Ok(reg) => reg,
        Err(e) => {
            raise_error!(
                "SCHEMA_REGISTRY_LOAD_CRITICAL",
                error = e,
                context = json_value!({
                    "space": space,
                    "db": db_name,
                    "hint": "Vérifiez la connexion réseau ou l'existence de la table des schémas"
                })
            )
        }
    };

    // 4. CHARGEMENT DE LA DONNÉE
    let data_full_path = dataset_root.join(&args.data);

    // REFACTOR : Lecture asynchrone, typée et sécurisée
    let mut doc: JsonValue = fs::read_json_async(&data_full_path).await?;

    // 5. VALIDATION
    let target_uri = &args.schema;
    let full_uri = registry.uri(target_uri);

    user_info!(
        "VALIDATOR_START",
        json_value!({
            "uri": full_uri,
            "protocol": "https",
            "active_domain": ctx.active_domain,
            "active_user": ctx.active_user
        })
    );

    let validator = match SchemaValidator::compile_with_registry(&full_uri, &registry) {
        Ok(v) => v,
        Err(e) => {
            raise_error!(
                "SCHEMA_COMPILATION_FAILED",
                error = e,
                context = json_value!({
                    "uri": full_uri,
                    "component": "SchemaValidator"
                })
            )
        }
    };

    match validator.compute_then_validate(&mut doc) {
        Ok(_) => {
            // 🎯 Conformité JSON stricte
            user_success!(
                "VALIDATOR_SUCCESS",
                json_value!({"status": "validation_passed"})
            );

            if let Some(id) = doc.get("_id") {
                user_info!("VALIDATOR_ID_GENERATED", json_value!({ "_id": id }));
            }
            Ok(())
        }
        Err(e) => {
            raise_error!(
                "VALIDATOR_FAILURE",
                error = e,
                context = json_value!({
                    "status": "validation_rejected",
                    "schema_uri": full_uri
                })
            )
        }
    }
}

// --- TESTS UNITAIRES ("Zéro Dette") ---
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
    fn test_validator_parsing() -> RaiseResult<()> {
        let args = vec!["test", "--data", "file.json", "--schema", "uri.json"];
        // 🎯 FIX DETTE : try_parse_from au lieu de parse_from (qui fait un panic)
        let cli = match TestCli::try_parse_from(args) {
            Ok(c) => c,
            Err(e) => raise_error!("ERR_TEST_PARSE", error = e.to_string()),
        };
        assert_eq!(cli.args.data, "file.json");
        assert_eq!(cli.args.schema, "uri.json");
        Ok(())
    }

    #[test]
    fn test_validator_missing_args() -> RaiseResult<()> {
        let args = vec!["test", "--data", "file.json"];
        let res = TestCli::try_parse_from(args);
        assert!(res.is_err());
        Ok(())
    }

    #[test]
    fn test_path_logic_robustness() -> RaiseResult<()> {
        let base = PathBuf::from("/tmp/raise");
        let sub = "data/test.json";
        let full = base.join(sub);
        assert!(full.to_string_lossy().contains("data/test.json"));
        Ok(())
    }
}
