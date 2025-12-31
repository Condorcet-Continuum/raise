use anyhow::{Context, Result};
use clap::Parser;
use dotenvy::dotenv;
use serde_json::Value;
use std::env;
use std::fs;
use std::path::PathBuf;

// Imports internes
use genaptitude::json_db::schema::{SchemaRegistry, SchemaValidator};
use genaptitude::json_db::storage::JsonDbConfig;

#[derive(Parser, Debug)]
#[command(author, version, about)]
struct Args {
    /// Chemin relatif du fichier de données DANS le dataset
    /// ex: data/dapps/tva-manager.json
    #[arg(short, long)]
    data: String,

    /// URI du schéma cible dans le registre
    /// ex: dapps/dapp.schema.json
    #[arg(short, long)]
    schema: String,
}

fn main() -> Result<()> {
    // 1. CHARGEMENT DU .ENV
    // dotenv() cherche le fichier .env dans le dossier courant et les parents
    if dotenv().is_err() {
        println!("⚠️  Attention: Fichier .env introuvable. On utilise les variables d'environnement système.");
    } else {
        println!("✅ Fichier .env chargé.");
    }

    let args = Args::parse();

    // 2. CONFIGURATION DES CHEMINS VIA ENV
    let domain_path_str = env::var("PATH_RAISE_DOMAIN")
        .context("❌ Variable 'PATH_RAISE_DOMAIN' manquante dans le .env")?;

    let dataset_path_str = env::var("PATH_RAISE_DATASET")
        .context("❌ Variable 'PATH_RAISE_DATASET' manquante dans le .env")?;

    let dataset_root = PathBuf::from(&dataset_path_str);
    let domain_root = PathBuf::from(&domain_path_str);

    // Vérification physique
    if !dataset_root.exists() {
        return Err(anyhow::anyhow!(
            "❌ Dossier Dataset introuvable : {:?}",
            dataset_root
        ));
    }
    if !domain_root.exists() {
        return Err(anyhow::anyhow!(
            "❌ Dossier Domain introuvable : {:?}",
            domain_root
        ));
    }

    // 3. CONFIGURATION DE LA DB (un2)
    // Par convention, la DB système est souvent dans <DOMAIN>/un2
    // On vérifie si "un2" existe dans le domaine, sinon on utilise la racine du domaine.
    let db_root = if domain_root.join("un2").exists() {
        domain_root.join("un2")
    } else {
        domain_root
    };

    println!("🔧 Config DB Root : {:?}", db_root);
    let cfg = JsonDbConfig::new(db_root);

    // 4. CHARGEMENT DU REGISTRE (Depuis la DB)
    // On charge l'espace "_system" et la db "schemas"
    let space = "_system";
    let db_name = "schemas";

    println!(
        "📦 Chargement des schémas depuis DB ({}/{}/v1)...",
        space, db_name
    );
    let registry = SchemaRegistry::from_db(&cfg, space, db_name)
        .context("Impossible de charger le registre des schémas depuis la DB")?;

    // 5. CHARGEMENT DE LA DONNÉE (Depuis le Dataset)
    let data_full_path = dataset_root.join(&args.data);
    println!("📂 Lecture donnée : {:?}", args.data);

    let content = fs::read_to_string(&data_full_path)
        .with_context(|| format!("Fichier de données introuvable : {:?}", data_full_path))?;

    let mut doc: Value =
        serde_json::from_str(&content).context("Erreur de parsing JSON du fichier de données")?;

    // 6. COMPILATION DU VALIDATEUR
    // On utilise l'URI relative passée en argument
    let target_uri = &args.schema;
    // On laisse le registre résoudre l'URI complète (souvent préfixée par db://...)
    let full_uri = registry.uri(target_uri);

    println!("📐 Compilation validateur pour : {}", full_uri);

    // Petit check de debug pour aider l'utilisateur si le schéma est introuvable
    if registry.get_by_uri(&full_uri).is_none() {
        println!("⚠️  ATTENTION : Schéma introuvable à l'URI '{}'", full_uri);
        println!("    Essayez de vérifier le préfixe ou le chemin relatif.");
    }

    let validator = SchemaValidator::compile_with_registry(&full_uri, &registry)
        .context("Échec de la compilation du SchemaValidator")?;

    // 7. COMPUTE & VALIDATE
    println!("🚀 Validation stricte (compute_then_validate)...");
    match validator.compute_then_validate(&mut doc) {
        Ok(_) => {
            println!("\n✅ SUCCÈS : Document VALIDE !");
            // Affiche les champs calculés pour preuve
            if let Some(id) = doc.get("id") {
                println!("   ID généré     : {}", id);
            }
            if let Some(at) = doc.get("createdAt") {
                println!("   Créé le       : {}", at);
            }
        }
        Err(e) => {
            println!("\n❌ ÉCHEC DE VALIDATION :");
            println!("{}", e);
            std::process::exit(1);
        }
    }

    Ok(())
}
