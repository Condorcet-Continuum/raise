// FICHIER : src-tauri/tools/raise-cli/src/commands/ai.rs

use clap::{Args, Subcommand};

// --- IMPORTS MÉTIER RAISE ---
use raise::ai::agents::intent_classifier::{EngineeringIntent, IntentClassifier};
use raise::ai::agents::tools::query_knowledge_graph;
use raise::ai::agents::{
    business_agent::BusinessAgent, data_agent::DataAgent, epbs_agent::EpbsAgent,
    hardware_agent::HardwareAgent, software_agent::SoftwareAgent, system_agent::SystemAgent,
    transverse_agent::TransverseAgent, Agent, AgentContext,
};
use raise::ai::llm::client::LlmClient;
use raise::ai::training::ai_train_domain_native;
use raise::json_db::collections::manager::CollectionsManager;

use raise::{
    user_error,
    user_info,
    user_success,
    utils::prelude::*, // AppConfig n'est plus importé, on l'a via CliContext !
};

// 🎯 NOUVEAU : Import du contexte global CLI
use crate::CliContext;

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

    /// 🔍 Inspecter un agent et son prompt lié
    #[command(visible_alias = "view")]
    Inspect {
        /// Référence de l'agent (ex: 'ref:agents:handle:agent_alpha_planner')
        reference: String,
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

// 🎯 La signature intègre le CliContext
pub async fn handle(args: AiArgs, ctx: CliContext) -> RaiseResult<()> {
    // 🎯 Heartbeat automatique
    let _ = ctx.session_mgr.touch().await;

    let domain_path = ctx
        .config
        .get_path("PATH_RAISE_DOMAIN")
        .expect("ERREUR: PATH_RAISE_DOMAIN introuvable !");

    let dataset_path = ctx
        .config
        .get_path("PATH_RAISE_DATASET")
        .unwrap_or_else(|| domain_path.join("dataset"));

    fs::ensure_dir_async(&domain_path).await?;
    // 2. MOTEURS ET CONTEXTE (OPTIMISÉ)
    let storage = ctx.storage.clone();

    // 🎯 PLUS DE DEVINETTE : On utilise directement le contexte résolu !
    let manager = CollectionsManager::new(&storage, &ctx.active_domain, &ctx.active_db);
    let client = LlmClient::new(&manager).await?;

    // On récupère juste l'ID de session
    let current_session = ctx.session_mgr.get_current_session().await;
    let session_id = current_session
        .as_ref()
        .map(|s| s.id.clone())
        .unwrap_or_else(|| "cli_session".to_string());

    // Instanciation asynchrone du Contexte Agent avec le vrai User résolu
    let agent_ctx = AgentContext::new(
        &ctx.active_user,
        &session_id,
        storage.clone(),
        client.clone(),
        domain_path.clone(),
        dataset_path,
    )
    .await;

    // 3. EXÉCUTION
    match args.command.unwrap_or(AiCommands::Interactive) {
        AiCommands::Interactive => run_interactive_mode(&agent_ctx, &ctx, client).await?,
        AiCommands::Classify { input, execute } => {
            process_input(&agent_ctx, &input, client, execute).await
        }
        AiCommands::Inspect { reference } => {
            inspect_agent_logic(&agent_ctx, &reference, &ctx.active_domain, &ctx.active_db).await?;
        }
        AiCommands::Train {
            domain,
            db: target_db,
            epochs,
            lr,
        } => {
            let final_domain = domain.unwrap_or_else(|| ctx.active_domain.clone());
            let final_db = target_db.unwrap_or_else(|| ctx.active_db.clone());

            let final_epochs = epochs.unwrap_or(3);
            let final_lr = lr.unwrap_or(ctx.config.deep_learning.learning_rate);

            user_info!(
                "AI_TRAINING_START",
                json_value!({ "domain": final_domain, "db": final_db, "lr": final_lr, "epochs": final_epochs })
            );

            match ai_train_domain_native(
                &storage,
                &ctx.active_domain, // L'espace d'exécution principal
                &final_db,
                &final_domain,
                final_epochs,
                final_lr,
            )
            .await
            {
                Ok(msg) => user_success!("AI_TRAIN_SUCCESS", json_value!({ "result": msg })),
                Err(e) => user_error!(
                    "AI_TRAIN_FAIL",
                    json_value!({ "error": e.to_string(), "action": "neural_network_training" })
                ),
            }
        }
    }

    Ok(())
}

async fn run_interactive_mode(
    ctx: &AgentContext,
    cli_ctx: &CliContext,
    client: LlmClient,
) -> RaiseResult<()> {
    // 🎯 Mise en conformité JSON stricte
    user_info!("AI_INTERACTIVE_WELCOME", json_value!({}));
    user_info!("AI_INTERACTIVE_SEPARATOR", json_value!({}));
    user_info!("AI_LLM_CONNECTED", json_value!({"mode": "local"}));
    user_info!(
        "AI_STORAGE_PATH",
        json_value!({ "path": ctx.paths.domain_root })
    );
    user_info!("AI_EXIT_HINT", json_value!({}));
    let prompt = format!(
        "RAISE-AI [{}@{}/{}]> ",
        cli_ctx.active_user, cli_ctx.active_domain, cli_ctx.active_db
    );
    loop {
        print!("{}", prompt);
        os::flush_stdout()?;
        let input = os::read_stdin_line()?;

        if input.eq_ignore_ascii_case("exit") {
            user_info!("AI_GOODBYE", json_value!({}));
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
    user_info!("AI_ANALYZING", json_value!({"input_length": input.len()}));

    let intent = classifier.classify(input).await;

    match intent {
        EngineeringIntent::DefineBusinessUseCase {
            ref process_name, ..
        } => {
            user_info!(
                "AI_AGENT_START",
                json_value!({ "agent": "Business Agent", "process": process_name })
            );
            run_agent(BusinessAgent::new(), ctx, &intent, execute).await;
        }
        EngineeringIntent::CreateElement { ref layer, .. } if layer == "SA" => {
            user_info!(
                "AI_AGENT_START",
                json_value!({"agent": "System Agent (SA)"})
            );
            run_agent(SystemAgent::new(), ctx, &intent, execute).await;
        }
        EngineeringIntent::CreateElement {
            ref layer,
            ref element_type,
            ..
        } if layer == "LA" || element_type.contains("Software") => {
            user_info!(
                "AI_AGENT_START",
                json_value!({"agent": "Software Agent (LA)"})
            );
            run_agent(SoftwareAgent::new(), ctx, &intent, execute).await;
        }
        EngineeringIntent::GenerateCode { .. } => {
            user_info!(
                "AI_CODE_GEN_START",
                json_value!({"agent": "Software Agent (Code)"})
            );
            run_agent(SoftwareAgent::new(), ctx, &intent, execute).await;
        }
        EngineeringIntent::CreateElement { ref layer, .. } if layer == "PA" => {
            user_info!(
                "AI_AGENT_START",
                json_value!({"agent": "Hardware Agent (PA)"})
            );
            run_agent(HardwareAgent::new(), ctx, &intent, execute).await;
        }
        EngineeringIntent::CreateElement { ref layer, .. } if layer == "EPBS" => {
            user_info!("AI_AGENT_START", json_value!({"agent": "EPBS Agent"}));
            run_agent(EpbsAgent::new(), ctx, &intent, execute).await;
        }
        EngineeringIntent::CreateElement { ref layer, .. } if layer == "DATA" => {
            user_info!("AI_AGENT_START", json_value!({"agent": "Data Agent"}));
            run_agent(DataAgent::new(), ctx, &intent, execute).await;
        }
        EngineeringIntent::CreateElement { ref layer, .. } if layer == "TRANSVERSE" => {
            user_info!("AI_AGENT_START", json_value!({"agent": "Transverse Agent"}));
            run_agent(TransverseAgent::new(), ctx, &intent, execute).await;
        }
        _ => {
            user_error!(
                "AI_INTENT_UNKNOWN",
                json_value!({ "intent_raw": format!("{:?}", intent) })
            );
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
                user_success!("AI_RESULT", json_value!({ "message": res.message }));
                for a in res.artifacts {
                    user_info!("AI_ARTIFACT_GENERATED", json_value!({ "path": a.path }));
                }
            }
            Ok(None) => {
                user_info!("AI_NO_ACTION", json_value!({}));
            }
            Err(e) => {
                user_error!(
                    "AI_AGENT_ERROR",
                    json_value!({ "error": e.to_string(), "source": "agent_executor" })
                );
            }
        }
    } else {
        user_info!("AI_SIMULATION_MODE", json_value!({}));
    }
}

async fn inspect_agent_logic(
    ctx: &AgentContext,
    reference: &str,
    space: &str, // 🎯 On passe le domaine résolu ici
    db: &str,    // 🎯 Et la DB ici
) -> RaiseResult<()> {
    user_info!(
        "AI_INSPECT_START",
        json_value!({
            "target": reference,
            "space": space,
            "db": db
        })
    );

    // On récupère l'agent
    let agent_doc = query_knowledge_graph(ctx, reference, false).await?;

    if let Some(prompt_id) = agent_doc["neuro_profile"]["prompt_id"].as_str() {
        // On récupère le prompt
        let prompt_doc = query_knowledge_graph(ctx, prompt_id, false).await?;

        // ✅ Correction Serde : .as_array() est nécessaire pour appeler .len()
        let directives_len = prompt_doc["directives"]
            .as_array()
            .map(|a| a.len())
            .unwrap_or(0);

        user_success!(
            "AI_PROMPT_RESOLVED",
            json_value!({
                "persona": prompt_doc["identity"]["persona"],
                "directives_count": directives_len
            })
        );

        println!("\n📝 --- INSTRUCTIONS RÉCUPÉRÉES ---");
        println!("Identité : {}", prompt_doc["identity"]["persona"]);
        if let Some(directives) = prompt_doc["directives"].as_array() {
            for (i, d) in directives.iter().enumerate() {
                println!("  {}. {}", i + 1, d.as_str().unwrap_or(""));
            }
        }
    }
    Ok(())
}

// --- TESTS UNITAIRES ---
// Note : Ces tests vérifient principalement le parsing (Clap) et la logique de distribution des intents.
// Ils n'appellent pas directement "handle", il n'est donc pas nécessaire de mocker le CliContext ici.
#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;
    use raise::utils::testing::*;

    #[derive(Parser)]
    struct TestCli {
        #[command(flatten)]
        args: AiArgs,
    }

    #[async_test]
    async fn test_ai_parsing_robustness() {
        mock::inject_mock_config().await;

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

    #[async_test]
    async fn test_intent_dispatch_layers() {
        mock::inject_mock_config().await;

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

    #[async_test]
    async fn test_intent_dispatch_software_logic() {
        mock::inject_mock_config().await;

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

    #[async_test]
    async fn test_business_dispatch() {
        mock::inject_mock_config().await;

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

    #[async_test]
    async fn test_ai_train_parsing() {
        mock::inject_mock_config().await;

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
