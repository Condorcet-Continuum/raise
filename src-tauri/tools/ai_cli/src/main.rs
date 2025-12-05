use anyhow::Result;
use clap::{Parser, Subcommand};
use dotenvy::dotenv;
use std::env;
use std::path::PathBuf;

// Imports Métier (Librairie GenAptitude)
use genaptitude::ai::agents::intent_classifier::{EngineeringIntent, IntentClassifier};
use genaptitude::ai::agents::{system_agent::SystemAgent, Agent};
use genaptitude::ai::llm::client::{LlmBackend, LlmClient};
use genaptitude::json_db::storage::{JsonDbConfig, StorageEngine};

/// Outil en ligne de commande (CLI) pour piloter le module IA de GenAptitude.
#[derive(Parser)]
#[command(
    name = "ai_cli",
    author = "GenAptitude Team",
    version,
    about = "Interface CLI pour le cerveau Neuro-Symbolique"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    #[command(visible_alias = "c")]
    Chat {
        message: String,
        #[arg(long, short = 'c')]
        cloud: bool,
    },
    #[command(visible_alias = "x")]
    Classify {
        input: String,
        #[arg(long, short = 'x')]
        execute: bool,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    // 1. Chargement Environnement
    dotenv().ok();

    // 2. Config IA & DB
    let gemini_key = env::var("GENAPTITUDE_GEMINI_KEY").unwrap_or_default();
    let model_name = env::var("GENAPTITUDE_MODEL_NAME").ok();
    let local_url =
        env::var("GENAPTITUDE_LOCAL_URL").unwrap_or_else(|_| "http://localhost:8080".to_string());

    let db_path_str =
        env::var("PATH_GENAPTITUDE_DOMAIN").unwrap_or_else(|_| "./genaptitude_db".to_string());
    let db_root = PathBuf::from(db_path_str);

    // Initialisation DB (déclenche l'auto-bootstrap des schémas si nécessaire)
    let config = JsonDbConfig::new(db_root);
    let storage = StorageEngine::new(config);

    // Initialisation Client LLM
    let client = LlmClient::new(&local_url, &gemini_key, model_name);

    let args = Cli::parse();

    match args.command {
        // --- COMMANDE CHAT ---
        Commands::Chat { message, cloud } => {
            let backend = if cloud {
                LlmBackend::GoogleGemini
            } else {
                LlmBackend::LocalLlama
            };
            let mode = if cloud { "CLOUD" } else { "LOCAL" };
            println!("🤖 [{}] Envoi : \"{}\"", mode, message);

            match client
                .ask(backend, "Tu es un assistant CLI.", &message)
                .await
            {
                Ok(response) => println!("\n✅ Réponse :\n{}", response),
                Err(e) => eprintln!("❌ Erreur : {}", e),
            }
        }

        // --- COMMANDE CLASSIFY ---
        Commands::Classify { input, execute } => {
            println!("🧠 Analyse : \"{}\"", input);

            let classifier = IntentClassifier::new(client.clone());
            let intent = classifier.classify(&input).await;

            // Affichage de l'intention brute pour debug
            println!("🔍 Intention détectée : {:?}", intent);

            match intent {
                // CAS 1 : CRÉATION D'ÉLÉMENT
                EngineeringIntent::CreateElement {
                    ref layer,
                    ref element_type,
                    ref name,
                } => {
                    println!("\n🔧 PLAN D'ACTION : CRÉATION");
                    println!("   • Cible : {} / {} / {}", layer, element_type, name);

                    if execute {
                        println!("⚡ Exécution SystemAgent...");
                        let agent = SystemAgent::new(client.clone(), storage);
                        match agent.process(&intent).await {
                            Ok(Some(res)) => println!("\n✅ SUCCÈS :\n{}", res),
                            Ok(None) => println!("\nℹ️ IGNORÉ : L'agent ne gère pas ce type."),
                            Err(e) => eprintln!("\n❌ ÉCHEC : {}", e),
                        }
                    } else {
                        println!("\n(Dry Run - Utilisez -x pour exécuter)");
                    }
                }

                // CAS 2 : CRÉATION DE RELATION (Nouveau)
                EngineeringIntent::CreateRelationship {
                    ref source_name,
                    ref target_name,
                    ref relation_type,
                } => {
                    println!("\n🔗 PLAN D'ACTION : RELIER");
                    println!("   • Source : {}", source_name);
                    println!("   • Cible  : {}", target_name);
                    println!("   • Type   : {}", relation_type);

                    if execute {
                        println!("⚡ Exécution SystemAgent...");
                        let agent = SystemAgent::new(client.clone(), storage);
                        match agent.process(&intent).await {
                            Ok(Some(res)) => println!("\n✅ SUCCÈS :\n{}", res),
                            Ok(None) => println!("\nℹ️ WIP : La gestion des relations n'est pas encore implémentée dans l'agent."),
                            Err(e) => eprintln!("\n❌ ÉCHEC : {}", e),
                        }
                    } else {
                        println!("\n(Dry Run - Utilisez -x pour exécuter)");
                    }
                }

                // CAS 3 : DISCUSSION
                EngineeringIntent::Chat => {
                    println!("\n💬 Mode DISCUSSION (Pas d'action technique)");
                }

                // CAS 4 : INCONNU
                EngineeringIntent::Unknown => {
                    println!("\n❓ INTENTION INCONNUE");
                }
            }
        }
    }

    Ok(())
}
