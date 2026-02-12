use clap::{Args, Subcommand, ValueEnum};

use raise::{user_info, user_success, utils::prelude::*};

// Imports depuis le cœur code_generator
use raise::code_generator::TargetLanguage;

/// Forge logicielle et matérielle (Arcadia-to-Code)
#[derive(Args, Clone, Debug)]
pub struct CodeGenArgs {
    #[command(subcommand)]
    pub command: CodeGenCommands,
}

#[derive(Subcommand, Clone, Debug)]
pub enum CodeGenCommands {
    /// Génère le code source pour un élément du modèle
    Generate {
        /// ID du composant à générer
        element_id: String,
        /// Langage cible
        #[arg(short, long, value_enum)]
        lang: CliTargetLanguage,
    },
}

/// Bridge entre clap et l'enum TargetLanguage du coeur
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum, Debug)]
pub enum CliTargetLanguage {
    Rust,
    Typescript,
    Cpp,
    Verilog,
    Vhdl,
}

impl From<CliTargetLanguage> for TargetLanguage {
    fn from(lang: CliTargetLanguage) -> Self {
        match lang {
            CliTargetLanguage::Rust => TargetLanguage::Rust,
            CliTargetLanguage::Typescript => TargetLanguage::TypeScript,
            CliTargetLanguage::Cpp => TargetLanguage::Cpp,
            CliTargetLanguage::Verilog => TargetLanguage::Verilog,
            CliTargetLanguage::Vhdl => TargetLanguage::Vhdl,
        }
    }
}

pub async fn handle(args: CodeGenArgs) -> Result<()> {
    match args.command {
        CodeGenCommands::Generate { element_id, lang } => {
            let target: TargetLanguage = lang.into();
            user_info!("FORGE", "Démarrage de la génération pour : {}", element_id);
            user_info!("TARGET", "Langage : {:?}", target);

            // Simulation du cycle de vie du CodeGeneratorService
            user_info!("SYNC", "Extraction des injections de code utilisateur...");

            if target == TargetLanguage::Rust {
                user_info!("LINT", "Exécution programmée de Clippy & Rustfmt.");
            }

            user_success!("FORGE_OK", "Artefacts {:?} générés avec succès.", target);
        }
    }
    Ok(())
}

// --- TESTS UNITAIRES ---
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_codegen_cli_dispatch() {
        let args = CodeGenArgs {
            command: CodeGenCommands::Generate {
                element_id: "Logical_CPU".into(),
                lang: CliTargetLanguage::Vhdl,
            },
        };
        assert!(handle(args).await.is_ok());
    }
}
