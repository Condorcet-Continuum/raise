use anyhow::Result;
use clap::{Parser, Subcommand};
use dotenvy::dotenv;
use std::env;
use std::io::{self, Write};
use std::path::PathBuf;
use std::sync::Arc;

// Imports Métier
use raise::ai::agents::intent_classifier::{EngineeringIntent, IntentClassifier};
use raise::ai::agents::{
    business_agent::BusinessAgent, data_agent::DataAgent, epbs_agent::EpbsAgent,
    hardware_agent::HardwareAgent, software_agent::SoftwareAgent, system_agent::SystemAgent,
    transverse_agent::TransverseAgent, Agent, AgentContext,
};

use raise::ai::llm::client::LlmClient;
use raise::json_db::storage::{JsonDbConfig, StorageEngine};

#[derive(Parser)]
#[command(
    name = "ai_cli",
    author = "RAISE Team",
    version,
    about = "Interface CLI pour le cerveau Neuro-Symbolique"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    #[command(visible_alias = "i")]
    Interactive,

    #[command(visible_alias = "x")]
    Classify {
        input: String,
        #[arg(long, short = 'x')]
        execute: bool,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenv().ok();

    let gemini_key = env::var("RAISE_GEMINI_KEY").unwrap_or_default();
    let model_name = env::var("RAISE_MODEL_NAME").ok();

    // --- CORRECTION CRITIQUE ICI ---
    // On cherche RAISE_LOCAL_URL (comme dans le .env) et non RAISE_LLM_LOCAL_URL
    let local_url =
        env::var("RAISE_LOCAL_URL").unwrap_or_else(|_| "http://localhost:8081".to_string());

    let domain_path = env::var("PATH_RAISE_DOMAIN")
        .map(PathBuf::from)
        .unwrap_or_else(|_| std::env::current_dir().unwrap().join("raise_storage"));

    let dataset_path = env::var("PATH_RAISE_DATASET")
        .map(PathBuf::from)
        .unwrap_or_else(|_| std::env::current_dir().unwrap().join("dataset"));

    std::fs::create_dir_all(&domain_path).ok();

    // On crée le client avec la bonne URL (celle du .env)
    let client = LlmClient::new(&local_url, &gemini_key, model_name.clone());

    let db_config = JsonDbConfig::new(domain_path.clone());
    let storage = StorageEngine::new(db_config);

    let ctx = AgentContext::new(
        Arc::new(storage),
        client.clone(),
        domain_path.clone(),
        dataset_path.clone(),
    );

    let cli = Cli::parse();

    match cli.command.unwrap_or(Commands::Interactive) {
        Commands::Interactive => {
            // On passe l'URL détectée pour vérification visuelle
            run_interactive_mode(&ctx, client, &local_url).await?;
        }

        Commands::Classify { input, execute } => {
            process_input(&ctx, &input, client, execute).await;
        }
    }

    Ok(())
}

async fn run_interactive_mode(
    ctx: &AgentContext,
    client: LlmClient,
    url_display: &str,
) -> Result<()> {
    println!("🤖 RAISE CLI (Mode Interactif)");
    println!("------------------------------------");
    println!("Analyseur NLP activé : Oui");
    println!("LLM Connecté : {}", url_display);
    println!("Stockage : {:?}", ctx.paths.domain_root);
    println!("\nExemple : 'Crée le Système de Pilotage'");
    println!("(Tapez 'exit' pour quitter)\n");

    loop {
        print!("RAISE> ");
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let input = input.trim();

        if input.eq_ignore_ascii_case("exit") {
            println!("Au revoir ! 👋");
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
    println!("🧠 Analyse...");

    let intent = classifier.classify(input).await;

    match intent {
        EngineeringIntent::DefineBusinessUseCase { ref domain, .. } => {
            println!("🚀 Business Agent ({})", domain);
            run_agent(BusinessAgent::new(), ctx, &intent, execute).await;
        }
        EngineeringIntent::CreateElement { ref layer, .. } if layer == "SA" => {
            println!("⚙️ System Agent (SA) + NLP");
            run_agent(SystemAgent::new(), ctx, &intent, execute).await;
        }
        EngineeringIntent::CreateElement {
            ref layer,
            ref element_type,
            ..
        } if layer == "LA" || element_type.contains("Software") => {
            println!("💻 Software Agent (LA)");
            run_agent(SoftwareAgent::new(), ctx, &intent, execute).await;
        }
        EngineeringIntent::GenerateCode { .. } => {
            println!("👨‍💻 Génération de Code");
            run_agent(SoftwareAgent::new(), ctx, &intent, execute).await;
        }
        EngineeringIntent::CreateElement { ref layer, .. } if layer == "PA" => {
            println!("🔧 Hardware Agent (PA)");
            run_agent(HardwareAgent::new(), ctx, &intent, execute).await;
        }
        EngineeringIntent::CreateElement { ref layer, .. } if layer == "EPBS" => {
            println!("📦 EPBS Agent");
            run_agent(EpbsAgent::new(), ctx, &intent, execute).await;
        }
        EngineeringIntent::CreateElement { ref layer, .. } if layer == "DATA" => {
            println!("💾 Data Agent");
            run_agent(DataAgent::new(), ctx, &intent, execute).await;
        }
        EngineeringIntent::CreateElement { ref layer, .. } if layer == "TRANSVERSE" => {
            println!("✨ Transverse Agent");
            run_agent(TransverseAgent::new(), ctx, &intent, execute).await;
        }
        _ => {
            println!("⚠️ Intention non comprise ou non gérée: {:?}", intent);
            println!("Essayez d'être plus précis (ex: 'Crée une fonction système X')");
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
                println!("\n✅ RÉSULTAT :");
                println!("{}", res.message);
                if !res.artifacts.is_empty() {
                    println!("📁 Fichiers :");
                    for a in res.artifacts {
                        println!("   - {}", a.path);
                    }
                }
                println!();
            }
            Ok(None) => println!("ℹ️ Agent: Aucune action nécessaire."),
            Err(e) => eprintln!("❌ Erreur Agent : {}", e),
        }
    } else {
        println!("(Simulation - Action ignorée)");
    }
}
