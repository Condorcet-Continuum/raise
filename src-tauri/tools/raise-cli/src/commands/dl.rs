// FICHIER : src-tauri/tools/raise-cli/src/commands/dl.rs

use clap::{Args, Subcommand};

use raise::{
    ai::deep_learning::api::{predict_semantic, train_model_semantic},
    json_db::collections::manager::CollectionsManager,
    raise_error, user_info, user_success,
    utils::prelude::*,
};

use crate::CliContext;

#[derive(Args, Debug, Clone)]
pub struct DlArgs {
    #[command(subcommand)]
    pub command: DlCommands,
}

#[derive(Subcommand, Debug, Clone)]
pub enum DlCommands {
    /// 🧠 Entraîne un modèle existant via son empreinte sémantique
    #[command(visible_alias = "t")]
    Train {
        /// Référence URN du modèle (ex: ref:dl_models:handle:routing_v1)
        urn: String,

        /// Valeurs d'entrée séparées par des virgules (ex: "0.5,1.2,-0.3")
        #[arg(long, short = 'i')]
        input: String,

        /// Classe cible attendue (ex: 1)
        #[arg(long, short = 'c')]
        target_class: u32,

        /// Nombre de passes (epochs) sur cet exemple
        #[arg(long, short = 'e', default_value = "1")]
        epochs: usize,
    },

    /// 🔮 Fait une prédiction via l'empreinte sémantique du modèle
    #[command(visible_alias = "p")]
    Predict {
        /// Référence URN du modèle (ex: ref:dl_models:handle:routing_v1)
        urn: String,

        /// Valeurs d'entrée séparées par des virgules (ex: "0.5,1.2,-0.3")
        #[arg(long, short = 'i')]
        input: String,
    },
}

pub async fn handle(args: DlArgs, ctx: CliContext) -> RaiseResult<()> {
    // 🎯 Création du manager pour naviguer dans l'Ontologie
    let manager = CollectionsManager::new(&ctx.storage, &ctx.active_domain, &ctx.active_db);

    match args.command {
        DlCommands::Train {
            urn,
            input,
            target_class,
            epochs,
        } => {
            let input_vec = parse_csv_floats(&input)?;
            user_info!(
                "DL_TRAIN_START",
                json_value!({"urn": urn, "epochs": epochs, "target": target_class})
            );

            // 🎯 L'intelligence sémantique est déléguée au moteur principal !
            let final_loss = train_model_semantic(
                &manager,
                &ctx.active_domain,
                &ctx.active_db,
                &urn,
                input_vec,
                target_class,
                epochs,
            )
            .await?;

            user_success!("DL_TRAIN_SUCCESS", json_value!({"final_loss": final_loss}));
        }

        DlCommands::Predict { urn, input } => {
            let input_vec = parse_csv_floats(&input)?;
            user_info!("DL_PREDICT_START", json_value!({"urn": urn}));

            let results = predict_semantic(
                &manager,
                &ctx.active_domain,
                &ctx.active_db,
                &urn,
                input_vec,
            )
            .await?;

            println!("\n📊 --- RÉSULTATS DE LA PRÉDICTION ---");
            for (class_idx, prob) in results.iter().enumerate() {
                println!("Classe {:>2} : {:.4}", class_idx, prob);
            }

            user_success!(
                "DL_PREDICT_SUCCESS",
                json_value!({"classes_count": results.len()})
            );
        }
    }

    Ok(())
}

/// Utilitaire pour parser une chaîne "0.1,0.2,0.3" en Vec<f32>
fn parse_csv_floats(input: &str) -> RaiseResult<Vec<f32>> {
    let mut vec = Vec::new();
    for part in input.split(',') {
        let val = match part.trim().parse::<f32>() {
            Ok(v) => v,
            Err(_) => {
                raise_error!(
                    "ERR_DL_PARSE_FLOAT",
                    error = format!("Valeur invalide: {}", part)
                )
            }
        };
        vec.push(val);
    }
    Ok(vec)
}

// =========================================================================
// TESTS UNITAIRES ET D'INTÉGRATION CLI
// =========================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;
    use raise::json_db::storage::{JsonDbConfig, StorageEngine};
    use raise::utils::context::SessionManager;
    use raise::utils::prelude::async_test;
    use raise::utils::testing::mock;

    #[test]
    fn test_parse_csv_floats_valid() -> RaiseResult<()> {
        let input = "0.5, -1.2 , 3.14  ,0";
        let result = match parse_csv_floats(input) {
            Ok(res) => res,
            Err(e) => raise_error!("ERR_TEST_PARSE", error = e.to_string()),
        };
        assert_eq!(result.len(), 4);
        assert_eq!(result[0], 0.5);
        Ok(())
    }

    #[test]
    fn test_parse_csv_floats_invalid() -> RaiseResult<()> {
        let input = "0.5, abc, 3.14";
        let result = parse_csv_floats(input);
        if result.is_ok() {
            raise_error!(
                "ERR_TEST_ASSERTION",
                error = "Le parseur devrait rejeter 'abc'"
            );
        }
        Ok(())
    }

    #[derive(Parser)]
    struct TestCli {
        #[command(flatten)]
        args: DlArgs,
    }

    #[test]
    fn test_dl_cli_parsing_train() -> RaiseResult<()> {
        let cli = match TestCli::try_parse_from(vec![
            "test",
            "train",
            "ref:dl_models:handle:routing_v1",
            "-i",
            "0.1,0.2",
            "-c",
            "2",
            "-e",
            "10",
        ]) {
            Ok(c) => c,
            Err(e) => raise_error!("ERR_TEST_CLI_PARSE", error = e.to_string()),
        };

        if let DlCommands::Train {
            urn,
            input,
            target_class,
            epochs,
        } = cli.args.command
        {
            assert_eq!(urn, "ref:dl_models:handle:routing_v1");
            assert_eq!(input, "0.1,0.2");
            assert_eq!(target_class, 2);
            assert_eq!(epochs, 10);
            Ok(())
        } else {
            raise_error!(
                "ERR_TEST_ASSERTION_FAILED",
                error = "Mauvaise commande parsée"
            )
        }
    }

    #[async_test]
    async fn test_dl_cli_semantic_workflow_e2e() -> RaiseResult<()> {
        mock::inject_mock_config().await;
        let config = AppConfig::get();
        let db_root = std::env::temp_dir().join("raise_test_dl_cli_semantic");
        let storage = SharedRef::new(StorageEngine::new(JsonDbConfig::new(db_root.clone())));
        let session_mgr = SessionManager::new(storage.clone());
        let ctx = CliContext::mock(config, session_mgr, storage.clone());

        // 1. Création d'un faux modèle dans la DB de test du CLI
        let model_doc: raise::utils::data::json::JsonValue = json_value!({
            "_id": "cli_model_123",
            "handle": "cli_routing_v1",
            "hyperparameters": { "input_size": 2, "hidden_size": 4, "output_size": 2, "learning_rate": 0.001 }
        });

        if let Err(e) = storage
            .write_document(
                &ctx.active_domain,
                &ctx.active_db,
                "dl_models",
                "cli_model_123",
                &model_doc,
            )
            .await
        {
            raise_error!("ERR_TEST_DB_WRITE", error = e.to_string());
        }

        let mock_input = "0.5, -0.5".to_string();
        let urn = "ref:dl_models:handle:cli_routing_v1".to_string();

        // 2. TRAIN via le CLI (L'Init se fait à la volée)
        let args_train = DlArgs {
            command: DlCommands::Train {
                urn: urn.clone(),
                input: mock_input.clone(),
                target_class: 1,
                epochs: 2,
            },
        };
        if let Err(e) = handle(args_train, ctx.clone()).await {
            raise_error!("ERR_TEST_TRAIN_FAIL", error = e.to_string());
        }

        // 3. PREDICT via le CLI
        let args_predict = DlArgs {
            command: DlCommands::Predict {
                urn: urn.clone(),
                input: mock_input,
            },
        };
        if let Err(e) = handle(args_predict, ctx.clone()).await {
            raise_error!("ERR_TEST_PREDICT_FAIL", error = e.to_string());
        }

        // Nettoyage
        let _ = raise::utils::io::fs::remove_dir_all_async(&db_root).await;
        Ok(())
    }
}
