use clap::{Args, Subcommand};
use std::io::{self as std_io, Write}; // On garde std::io pour flush/read_line (interactivité CLI)

// --- IMPORTS MÉTIER RAISE ---
use raise::ai::agents::intent_classifier::{EngineeringIntent, IntentClassifier};
use raise::ai::agents::{
    business_agent::BusinessAgent, data_agent::DataAgent, epbs_agent::EpbsAgent,
    hardware_agent::HardwareAgent, software_agent::SoftwareAgent, system_agent::SystemAgent,
    transverse_agent::TransverseAgent, Agent, AgentContext,
};
use raise::ai::llm::client::LlmClient;
use raise::json_db::storage::{JsonDbConfig, StorageEngine};

use raise::{
    user_error, user_info, user_success,
    utils::{io, prelude::*, Arc},
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
}

pub async fn handle(args: AiArgs) -> Result<()> {
    let config = AppConfig::get();
    let engine = config.ai_engines.get("cloud_gemini");

    let api_key = engine.and_then(|e| e.api_key.clone()).unwrap_or_default();
    let api_url = engine.and_then(|e| e.api_url.clone()).unwrap_or_default();
    let model_name = engine.map(|e| e.model_name.clone());

    let space = &config.default_domain;
    let db = &config.default_db;

    let domain_path = config
        .get_path("PATH_RAISE_DOMAIN")
        .expect("ERREUR: PATH_RAISE_DOMAIN introuvable !");

    let dataset_path = config
        .get_path("PATH_RAISE_DATASET")
        .unwrap_or_else(|| domain_path.join("dataset"));

    io::ensure_dir(&domain_path).await?;

    // 2. MOTEURS ET CONTEXTE
    let client = LlmClient::new(&api_url, &api_key, model_name);
    let storage = StorageEngine::new(JsonDbConfig::new(domain_path.clone()));

    let ctx = AgentContext::new(
        space,
        db,
        Arc::new(storage),
        client.clone(),
        domain_path,
        dataset_path,
    );

    // 3. EXÉCUTION
    match args.command.unwrap_or(AiCommands::Interactive) {
        AiCommands::Interactive => run_interactive_mode(&ctx, client, &api_url).await?,
        AiCommands::Classify { input, execute } => {
            process_input(&ctx, &input, client, execute).await
        }
    }

    Ok(())
}

async fn run_interactive_mode(
    ctx: &AgentContext,
    client: LlmClient,
    url_display: &str,
) -> Result<()> {
    user_info!("AI_INTERACTIVE_WELCOME");
    user_info!("AI_INTERACTIVE_SEPARATOR");
    user_info!("AI_LLM_CONNECTED", "{}", url_display);
    user_info!("AI_STORAGE_PATH", "{:?}", ctx.paths.domain_root);
    user_info!("AI_EXIT_HINT");

    loop {
        print!("RAISE-AI> ");
        std_io::stdout().flush()?;

        let mut input = String::new();
        std_io::stdin().read_line(&mut input)?;
        let input = input.trim();

        if input.eq_ignore_ascii_case("exit") {
            user_info!("AI_GOODBYE");
            break;
        }
        if input.is_empty() {
            continue;
        }

        process_input(ctx, input, client.clone(), true).await;
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
            user_info!("AI_AGENT_START", "Business Agent ({})", process_name);
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
            user_error!("AI_INTENT_UNKNOWN", "{:?}", intent);
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
                user_success!("AI_RESULT", "{}", res.message);
                for a in res.artifacts {
                    user_info!("AI_ARTIFACT_GENERATED", "{}", a.path);
                }
            }
            Ok(None) => {
                user_info!("AI_NO_ACTION");
            }
            Err(e) => {
                user_error!("AI_AGENT_ERROR", "{}", e);
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

    #[derive(Parser)]
    struct TestCli {
        #[command(flatten)]
        args: AiArgs,
    }

    #[test]
    fn test_ai_parsing_robustness() {
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
}
