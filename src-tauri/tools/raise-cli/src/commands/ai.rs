// FICHIER : src-tauri/tools/raise-cli/src/commands/ai.rs

use clap::{Args, Subcommand};
//use std::io::{self as std_io};

// --- IMPORTS MÉTIER RAISE ---
use raise::ai::agents::intent_classifier::{EngineeringIntent, IntentClassifier};
use raise::ai::agents::{
    business_agent::BusinessAgent, data_agent::DataAgent, epbs_agent::EpbsAgent,
    hardware_agent::HardwareAgent, software_agent::SoftwareAgent, system_agent::SystemAgent,
    transverse_agent::TransverseAgent, Agent, AgentContext,
};
use raise::ai::llm::client::LlmClient;
use raise::ai::training::ai_train_domain_native;
use raise::json_db::storage::{JsonDbConfig, StorageEngine};
// 🎯 NOUVEAU : Import du manager
use raise::json_db::collections::manager::CollectionsManager;

use raise::{
    user_error, user_info, user_success,
    utils::{config::AppConfig, io, os, prelude::*, Arc},
};

#[derive(Args, Debug, Clone)]
pub struct AiArgs {
    #[command(subcommand)]
    pub command: Option<AiCommands>,
}

#[derive(Subcommand, Debug, Clone)]
pub enum AiCommands {
    /// Mode interactif avec le cerveau RAISE
    #[command(visible_alias = "i")]
    Interactive,

    /// Classifier une intention et éventuellement l'exécuter
    #[command(visible_alias = "x")]
    Classify {
        input: String,
        #[arg(long, short = 'x')]
        execute: bool,
    },

    /// 🧠 Entraîne un adaptateur LoRA pour un domaine spécifique en local
    #[command(visible_alias = "t")]
    Train {
        /// Forcer le domaine à entraîner (écrase la config utilisateur)
        #[arg(short, long)]
        domain: Option<String>,

        /// Forcer la DB à utiliser
        #[arg(long)]
        db: Option<String>,

        /// Forcer le nombre d'époques (ex: 5)
        #[arg(short, long)]
        epochs: Option<usize>,

        /// Forcer le taux d'apprentissage (ex: 0.001)
        #[arg(short, long)]
        lr: Option<f64>,
    },
}

pub async fn handle(args: AiArgs) -> RaiseResult<()> {
    let config = AppConfig::get();

    let space = &config.system_domain;
    let db = &config.system_db;

    let domain_path = config
        .get_path("PATH_RAISE_DOMAIN")
        .expect("ERREUR: PATH_RAISE_DOMAIN introuvable !");

    let dataset_path = config
        .get_path("PATH_RAISE_DATASET")
        .unwrap_or_else(|| domain_path.join("dataset"));

    io::ensure_dir(&domain_path).await?;

    // 2. MOTEURS ET CONTEXTE (OPTIMISÉ)
    let storage = Arc::new(StorageEngine::new(JsonDbConfig::new(domain_path.clone())));

    // 🎯 Création du manager pour le CLI
    let manager = CollectionsManager::new(&storage, space, db);

    // 🎯 Instanciation asynchrone du LLM avec le manager
    let client = LlmClient::new(&manager).await?;

    // 🎯 Instanciation asynchrone du Contexte Agent
    let ctx = AgentContext::new(
        "cli_user",
        "cli_session",
        storage.clone(),
        client.clone(),
        domain_path.clone(),
        dataset_path,
    )
    .await;

    // 3. EXÉCUTION
    match args.command.unwrap_or(AiCommands::Interactive) {
        AiCommands::Interactive => run_interactive_mode(&ctx, client).await?,
        AiCommands::Classify { input, execute } => {
            process_input(&ctx, &input, client, execute).await
        }
        AiCommands::Train {
            domain,
            db: target_db,
            epochs,
            lr,
        } => {
            // 🎯 Résolution dynamique simple (l'objet ScopeConfig a été allégé)
            let final_domain = domain
                .or_else(|| config.user.as_ref().and_then(|u| u.default_domain.clone()))
                .or_else(|| {
                    config
                        .workstation
                        .as_ref()
                        .and_then(|w| w.default_domain.clone())
                })
                .unwrap_or_else(|| config.system_domain.clone());

            let final_db = target_db
                .or_else(|| config.user.as_ref().and_then(|u| u.default_db.clone()))
                .or_else(|| {
                    config
                        .workstation
                        .as_ref()
                        .and_then(|w| w.default_db.clone())
                })
                .unwrap_or_else(|| config.system_db.clone());

            // 🎯 Les valeurs par défaut de l'IA sont gérées plus simplement
            let final_epochs = epochs.unwrap_or(3);
            let final_lr = lr.unwrap_or(config.deep_learning.learning_rate);
            user_info!(
                "AI_TRAINING_START",
                json!({ "domain": final_domain, "db": final_db, "lr": final_lr, "epochs": final_epochs })
            );

            let train_storage =
                StorageEngine::new(JsonDbConfig::new(domain_path.join(space).join(&final_db)));

            match ai_train_domain_native(
                &train_storage,
                space,
                &final_db,
                &final_domain,
                final_epochs,
                final_lr,
            )
            .await
            {
                Ok(msg) => user_success!("AI_TRAIN_SUCCESS", json!({ "result": msg })),
                Err(e) => user_error!(
                    "AI_TRAIN_FAIL",
                    json!({ "error": e.to_string(), "action": "neural_network_training" })
                ),
            }
        }
    }

    Ok(())
}

async fn run_interactive_mode(ctx: &AgentContext, client: LlmClient) -> RaiseResult<()> {
    user_info!("AI_INTERACTIVE_WELCOME");
    user_info!("AI_INTERACTIVE_SEPARATOR");
    user_info!("AI_LLM_CONNECTED", "local");
    user_info!("AI_STORAGE_PATH", json!({ "path": ctx.paths.domain_root }));
    user_info!("AI_EXIT_HINT");

    loop {
        print!("RAISE-AI> ");
        os::flush_stdout()?;
        let input = os::read_stdin_line()?;

        if input.eq_ignore_ascii_case("exit") {
            user_info!("AI_GOODBYE");
            break;
        }
        if input.is_empty() {
            continue;
        }

        process_input(ctx, &input, client.clone(), true).await;
    }
    Ok(())
}

async fn process_input(ctx: &AgentContext, input: &str, client: LlmClient, execute: bool) {
    let classifier = IntentClassifier::new(client);
    user_info!("AI_ANALYZING");

    let intent = classifier.classify(input).await;

    match intent {
        EngineeringIntent::DefineBusinessUseCase {
            ref process_name, ..
        } => {
            user_info!(
                "AI_AGENT_START",
                json!({ "agent": "Business Agent", "process": process_name })
            );
            run_agent(BusinessAgent::new(), ctx, &intent, execute).await;
        }
        EngineeringIntent::CreateElement { ref layer, .. } if layer == "SA" => {
            user_info!("AI_AGENT_START", "System Agent (SA)");
            run_agent(SystemAgent::new(), ctx, &intent, execute).await;
        }
        EngineeringIntent::CreateElement {
            ref layer,
            ref element_type,
            ..
        } if layer == "LA" || element_type.contains("Software") => {
            user_info!("AI_AGENT_START", "Software Agent (LA)");
            run_agent(SoftwareAgent::new(), ctx, &intent, execute).await;
        }
        EngineeringIntent::GenerateCode { .. } => {
            user_info!("AI_CODE_GEN_START");
            run_agent(SoftwareAgent::new(), ctx, &intent, execute).await;
        }
        EngineeringIntent::CreateElement { ref layer, .. } if layer == "PA" => {
            user_info!("AI_AGENT_START", "Hardware Agent (PA)");
            run_agent(HardwareAgent::new(), ctx, &intent, execute).await;
        }
        EngineeringIntent::CreateElement { ref layer, .. } if layer == "EPBS" => {
            user_info!("AI_AGENT_START", "EPBS Agent");
            run_agent(EpbsAgent::new(), ctx, &intent, execute).await;
        }
        EngineeringIntent::CreateElement { ref layer, .. } if layer == "DATA" => {
            user_info!("AI_AGENT_START", "Data Agent");
            run_agent(DataAgent::new(), ctx, &intent, execute).await;
        }
        EngineeringIntent::CreateElement { ref layer, .. } if layer == "TRANSVERSE" => {
            user_info!("AI_AGENT_START", "Transverse Agent");
            run_agent(TransverseAgent::new(), ctx, &intent, execute).await;
        }
        _ => {
            user_error!("AI_INTENT_UNKNOWN", json!({ "intent_raw": intent }));
        }
    }
}

async fn run_agent<A: Agent>(
    agent: A,
    ctx: &AgentContext,
    intent: &EngineeringIntent,
    execute: bool,
) {
    if execute {
        match agent.process(ctx, intent).await {
            Ok(Some(res)) => {
                user_success!("AI_RESULT", json!({ "message": res.message }));
                for a in res.artifacts {
                    user_info!("AI_ARTIFACT_GENERATED", json!({ "path": a.path }));
                }
            }
            Ok(None) => {
                user_info!("AI_NO_ACTION");
            }
            Err(e) => {
                user_error!(
                    "AI_AGENT_ERROR",
                    json!({ "error": e.to_string(), "source": "agent_executor" })
                );
            }
        }
    } else {
        user_info!("AI_SIMULATION_MODE");
    }
}

// --- TESTS UNITAIRES ---
#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;
    use raise::utils::config::test_mocks;

    #[derive(Parser)]
    struct TestCli {
        #[command(flatten)]
        args: AiArgs,
    }

    #[test]
    fn test_ai_parsing_robustness() {
        test_mocks::inject_mock_config();

        let cli = TestCli::parse_from(vec!["test"]);
        assert!(cli.args.command.is_none());

        let cli = TestCli::parse_from(vec![
            "test",
            "classify",
            "créer un composant SA",
            "--execute",
        ]);
        if let Some(AiCommands::Classify { input, execute }) = cli.args.command {
            assert_eq!(input, "créer un composant SA");
            assert!(execute);
        } else {
            panic!("Échec du parsing de la commande Classify");
        }
    }

    #[test]
    fn test_intent_dispatch_layers() {
        test_mocks::inject_mock_config();

        let test_cases = vec![
            ("SA", "System Agent"),
            ("PA", "Hardware Agent"),
            ("DATA", "Data Agent"),
            ("TRANSVERSE", "Transverse Agent"),
            ("EPBS", "EPBS Agent"),
        ];

        for (layer, expected_name) in test_cases {
            let intent = EngineeringIntent::CreateElement {
                layer: layer.to_string(),
                element_type: "Component".into(),
                name: "TestUnit".into(),
            };

            let matched_agent = match intent {
                EngineeringIntent::CreateElement { ref layer, .. } if layer == "SA" => {
                    "System Agent"
                }
                EngineeringIntent::CreateElement { ref layer, .. } if layer == "PA" => {
                    "Hardware Agent"
                }
                EngineeringIntent::CreateElement { ref layer, .. } if layer == "DATA" => {
                    "Data Agent"
                }
                EngineeringIntent::CreateElement { ref layer, .. } if layer == "TRANSVERSE" => {
                    "Transverse Agent"
                }
                EngineeringIntent::CreateElement { ref layer, .. } if layer == "EPBS" => {
                    "EPBS Agent"
                }
                _ => "Other",
            };

            assert_eq!(matched_agent, expected_name);
        }
    }

    #[test]
    fn test_intent_dispatch_software_logic() {
        test_mocks::inject_mock_config();

        let intent_la = EngineeringIntent::CreateElement {
            layer: "LA".into(),
            element_type: "LogicalComponent".into(),
            name: "Test".into(),
        };

        let is_software = match intent_la {
            EngineeringIntent::CreateElement { ref layer, .. } if layer == "LA" => true,
            _ => false,
        };

        assert!(is_software);
    }

    #[test]
    fn test_business_dispatch() {
        test_mocks::inject_mock_config();

        let intent = EngineeringIntent::DefineBusinessUseCase {
            domain: "Aéronautique".into(),
            process_name: "Gestion Flux".into(),
            description: "Flux passagers".into(),
        };

        let is_business = match intent {
            EngineeringIntent::DefineBusinessUseCase { .. } => true,
            _ => false,
        };

        assert!(is_business);
    }

    #[test]
    fn test_ai_train_parsing() {
        test_mocks::inject_mock_config();

        let cli = TestCli::parse_from(vec![
            "test", "train", "--domain", "safety", "--epochs", "10",
        ]);

        if let Some(AiCommands::Train {
            domain,
            epochs,
            db,
            lr,
        }) = cli.args.command
        {
            assert_eq!(domain.unwrap(), "safety");
            assert_eq!(epochs.unwrap(), 10);
            assert!(db.is_none());
            assert!(lr.is_none());
        } else {
            panic!("Échec du parsing de la commande Train");
        }
    }
}
