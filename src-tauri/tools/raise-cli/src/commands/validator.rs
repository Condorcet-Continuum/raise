use clap::Args;

use raise::{
    user_info, user_success,
    utils::{
        data::Value,
        io::{self},
        prelude::*,
    },
};

use raise::json_db::schema::{SchemaRegistry, SchemaValidator};
use raise::json_db::storage::JsonDbConfig;

#[derive(Args, Debug, Clone)]
pub struct ValidatorArgs {
    /// Chemin relatif du fichier de données DANS le dataset (ex: data/dapps/tva-manager.json)
    #[arg(short, long)]
    pub data: String,

    /// URI du schéma cible dans le registre (ex: dapps/dapp.schema.json)
    #[arg(short, long)]
    pub schema: String,
}

pub async fn handle(args: ValidatorArgs) -> RaiseResult<()> {
    // 1. RÉCUPÉRATION DE LA CONFIGURATION
    let app_config = AppConfig::get();

    let domain_root = app_config
        .get_path("PATH_RAISE_DOMAIN")
        .expect("ERREUR: Le chemin PATH_RAISE_DOMAIN est introuvable !");

    // Chemin DATASET (Piloté par la config globale avec fallback sur le domaine)
    let dataset_root = app_config
        .get_path("PATH_RAISE_DATASET")
        .unwrap_or_else(|| domain_root.join("dataset"));

    // Vérification physique des dossiers racines
    if !dataset_root.exists() {
        raise_error!(
            "FS_DIRECTORY_MISSING",
            error = "Dataset root folder not found",
            context = json!({
                "path": dataset_root,
                "required_for": "data_persistence"
            })
        );
    }

    if !domain_root.exists() {
        raise_error!(
            "FS_DIRECTORY_MISSING",
            error = "Domain root folder not found",
            context = json!({
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
        json!({
            "space": space,
            "db": db_name,
            "action": "schema_fetch"
        })
    );

    let registry = match SchemaRegistry::from_db(&cfg, space, db_name).await {
        Ok(reg) => reg,
        Err(e) => {
            raise_error!(
                "SCHEMA_REGISTRY_LOAD_CRITICAL",
                error = e,
                context = json!({
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
    // Plus besoin de fs::read_to_string manuel ni de serde_json::from_str
    let mut doc: Value = io::read_json(&data_full_path).await?;

    // 5. VALIDATION
    let target_uri = &args.schema;
    let full_uri = registry.uri(target_uri);

    user_info!(
        "VALIDATOR_START",
        json!({ "uri": full_uri, "protocol": "https" })
    );

    let validator = match SchemaValidator::compile_with_registry(&full_uri, &registry) {
        Ok(v) => v,
        Err(e) => {
            // Ici raise_error! peut faire son 'return Err(...)' au niveau de la fonction
            raise_error!(
                "SCHEMA_COMPILATION_FAILED",
                error = e,
                context = json!({
                    "uri": full_uri,
                    "component": "SchemaValidator"
                })
            )
        }
    };

    match validator.compute_then_validate(&mut doc) {
        Ok(_) => {
            user_success!("VALIDATOR_SUCCESS");

            if let Some(id) = doc.get("id") {
                // Utilisation du Cas 2 de user_info! (Contexte structuré)
                user_info!("VALIDATOR_ID_GENERATED", json!({ "id": id }));
            }
            Ok(())
        }
        Err(e) => {
            // Remplacement de user_error! + Err(AppError) par raise_error!
            // Cela logue l'erreur technique ET retourne un AppError::Structured
            raise_error!(
                "VALIDATOR_FAILURE",
                error = e,
                context = json!({
                    "status": "validation_rejected",
                    "schema_uri": full_uri
                })
            )
        }
    }
}

// --- TESTS UNITAIRES (Patrimoine Conservé) ---
#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;
    use raise::utils::io::PathBuf;

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
