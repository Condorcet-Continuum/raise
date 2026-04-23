// FICHIER : src-tauri/tools/raise-cli/src/commands/dl.rs

use clap::{Args, Subcommand};
use raise::{
    ai::deep_learning::api::{predict_semantic, train_model_semantic},
    json_db::collections::manager::CollectionsManager,
    user_info,
    user_success,
    utils::prelude::*, // 🎯 Façade Unique RAISE
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
        /// Valeurs d'entrée séparées par des virgules (ex: "0.5,1.2")
        #[arg(long, short = 'i')]
        input: String,
        /// Classe cible attendue (ex: 1)
        #[arg(long, short = 'c')]
        target_class: u32,
        /// Nombre d'époques
        #[arg(long, short = 'e', default_value = "1")]
        epochs: usize,
    },
    /// 🔮 Inférence via l'empreinte sémantique du modèle
    #[command(visible_alias = "p")]
    Predict {
        /// Référence URN du modèle
        urn: String,
        /// Valeurs d'entrée (ex: "0.5,1.2")
        #[arg(long, short = 'i')]
        input: String,
    },
}

pub async fn handle(args: DlArgs, ctx: CliContext) -> RaiseResult<()> {
    // 🎯 Heartbeat de session
    let _ = ctx.session_mgr.touch().await;

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
                json_value!({"urn": &urn, "epochs": epochs})
            );

            // 🎯 Délégation sémantique au moteur Natif (Forteresse Inférence)
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
            user_info!("DL_PREDICT_START", json_value!({"urn": &urn}));

            let results = predict_semantic(
                &manager,
                &ctx.active_domain,
                &ctx.active_db,
                &urn,
                input_vec,
            )
            .await?;

            println!("\n📊 --- RÉSULTATS DE L'INFÉRENCE ---");
            for (idx, prob) in results.iter().enumerate() {
                println!("Classe {:>2} : {:.4}", idx, prob);
            }

            user_success!(
                "DL_PREDICT_SUCCESS",
                json_value!({"classes_count": results.len()})
            );
        }
    }
    Ok(())
}

fn parse_csv_floats(input: &str) -> RaiseResult<Vec<f32>> {
    let mut vec = Vec::new();
    for part in input.split(',') {
        let val = match part.trim().parse::<f32>() {
            Ok(v) => v,
            Err(_) => {
                // 🎯 FIX : La macro diverge, pas de return
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
// TESTS UNITAIRES (Rigueur VRAM & Sandbox)
// =========================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use raise::utils::testing::DbSandbox;

    #[test]
    #[serial_test::serial]
    fn test_parse_csv_floats_integrity() -> RaiseResult<()> {
        let result = parse_csv_floats("0.5, -1.2, 3.14")?;
        assert_eq!(result.len(), 3);
        assert_eq!(result[0], 0.5);
        Ok(())
    }

    /// 🎯 TEST E2E : Validation du workflow DL avec contrainte VRAM
    #[async_test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_dl_cli_workflow_deterministic() -> RaiseResult<()> {
        // 🎯 FIX CRITIQUE : Initialisation du registre sémantique pour les tests
        // Nécessaire pour éviter la panique "VocabularyRegistry non initialisé"
        raise::json_db::jsonld::VocabularyRegistry::init_mock_for_tests();

        let sandbox = DbSandbox::new().await?;
        let storage = SharedRef::new(sandbox.storage.clone());
        let session_mgr = crate::context::SessionManager::new(storage.clone());

        let ctx = crate::CliContext::mock(AppConfig::get(), session_mgr, storage);
        let manager = CollectionsManager::new(&ctx.storage, &ctx.active_domain, &ctx.active_db);

        // Initialisation physique de la base de données
        DbSandbox::mock_db(&manager).await?;

        // 1. Inscription d'un modèle sémantique
        let model_doc = json_value!({
            "_id": "model_test_001",
            "handle": "routing_v1",
            "hyperparameters": {
                "input_size": 2,
                "hidden_size": 4,
                "output_size": 2,
                "learning_rate": 0.01
            }
        });

        manager
            .create_collection(
                "dl_models",
                "db://_system/_system/schemas/v1/db/generic.schema.json",
            )
            .await?;

        manager.upsert_document("dl_models", model_doc).await?;

        // 2. Exécution Train
        let args_train = DlArgs {
            command: DlCommands::Train {
                urn: "ref:dl_models:handle:routing_v1".into(),
                input: "0.5, -0.5".into(),
                target_class: 1,
                epochs: 1,
            },
        };
        handle(args_train, ctx.clone()).await?;

        // 3. Exécution Predict
        let args_predict = DlArgs {
            command: DlCommands::Predict {
                urn: "ref:dl_models:handle:routing_v1".into(),
                input: "0.5, -0.5".into(),
            },
        };
        handle(args_predict, ctx).await
    }
}
