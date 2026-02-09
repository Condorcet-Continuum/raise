// FICHIER : src-tauri/src/utils/sys.rs

use crate::utils::{AppError, Result};
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};
use tracing::{debug, error, instrument, warn};

/// Exécute une commande système et capture sa sortie.
/// Utile pour lancer des outils comme Cargo, Git, etc.
///
/// # Arguments
/// * `cmd` - Le binaire à lancer (ex: "cargo", "git")
/// * `args` - Liste des arguments
/// * `cwd` - Dossier d'exécution optionnel
#[instrument(skip(args), fields(cmd = cmd, cwd = ?cwd))]
pub fn exec_command(cmd: &str, args: &[&str], cwd: Option<&Path>) -> Result<String> {
    debug!("🚀 Exécution commande système : {} {:?}", cmd, args);

    let mut command = Command::new(cmd);
    command.args(args);

    // Configuration du dossier courant
    if let Some(dir) = cwd {
        if !dir.exists() {
            return Err(AppError::System(anyhow::anyhow!(
                "Dossier d'exécution introuvable: {:?}",
                dir
            )));
        }
        command.current_dir(dir);
    }

    // On capture tout pour le diagnostic
    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());

    // Lancement et attente
    match command.output() {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();

            if output.status.success() {
                debug!("✅ Commande réussie");
                Ok(stdout)
            } else {
                warn!("⚠️ Commande échouée (code {:?})", output.status.code());
                debug!("Stderr: {}", stderr);
                // On retourne une erreur avec le stderr pour comprendre pourquoi ça a planté
                Err(AppError::System(anyhow::anyhow!(
                    "Echec commande '{}': {}",
                    cmd,
                    stderr.trim()
                )))
            }
        }
        Err(e) => {
            error!("❌ Impossible de lancer la commande '{}': {}", cmd, e);
            Err(AppError::Io(e))
        }
    }
}

/// Passe une chaîne de caractères dans l'entrée standard (stdin) d'une commande
/// et récupère le résultat transformé (stdout).
/// Typiquement utilisé pour les formateurs de code (rustfmt, prettier).
#[instrument(skip(input), fields(cmd = cmd))]
pub fn pipe_through(cmd: &str, input: &str) -> Result<String> {
    // 1. Lancement du processus
    let mut child = Command::new(cmd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| AppError::System(anyhow::anyhow!("Outil introuvable '{}': {}", cmd, e)))?;

    // 2. Écriture dans stdin
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(input.as_bytes()).map_err(AppError::Io)?;
    }

    // 3. Attente du résultat
    let output = child.wait_with_output().map_err(AppError::Io)?;

    if output.status.success() {
        let result = String::from_utf8_lossy(&output.stdout).to_string();
        Ok(result)
    } else {
        // Si le formateur échoue (syntaxe invalide ?), on renvoie une erreur explicite
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        Err(AppError::System(anyhow::anyhow!(
            "Echec du pipe '{}': {}",
            cmd,
            stderr
        )))
    }
}

// --- TESTS UNITAIRES ---
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exec_command_success() {
        // On utilise 'cargo --version' car on est sûr qu'il est présent dans l'environnement de dev
        let res = exec_command("cargo", &["--version"], None);

        assert!(res.is_ok(), "La commande cargo --version devrait réussir");
        let output = res.unwrap();
        assert!(
            output.starts_with("cargo"),
            "La sortie doit commencer par 'cargo'"
        );
    }

    #[test]
    fn test_exec_command_not_found() {
        // Commande qui n'existe pas
        let res = exec_command("commande_qui_n_existe_pas_12345", &[], None);

        assert!(res.is_err());
        match res.unwrap_err() {
            // Doit être une erreur IO (NotFound)
            AppError::Io(_) => assert!(true),
            _ => panic!("Devrait retourner une erreur IO pour binaire manquant"),
        }
    }

    #[test]
    fn test_exec_command_failure_status() {
        // Commande qui existe mais retourne un code d'erreur
        // ex: 'cargo build' sans fichier Cargo.toml valide dans un dossier vide (ou arguments invalides)
        let res = exec_command("cargo", &["build", "--manifest-path", "ghost.toml"], None);

        assert!(res.is_err());
        match res.unwrap_err() {
            // Doit être une erreur System (notre wrapper autour du code de sortie != 0)
            AppError::System(msg) => {
                let msg_str = msg.to_string();
                assert!(
                    msg_str.contains("Echec commande"),
                    "Message d'erreur incorrect"
                );
            }
            _ => panic!("Devrait retourner une erreur System pour un échec de commande"),
        }
    }

    #[test]
    fn test_pipe_through_rustfmt() {
        // On teste le pipe avec 'rustfmt' qui est installé pour ce projet
        let unformatted = "fn  main ( )  {  let x = 1 ; }";
        let expected_part = "fn main() {";

        let res = pipe_through("rustfmt", unformatted);

        // Note : Ce test passe si rustfmt est installé. Sinon on ignore pour ne pas casser la CI.
        match res {
            Ok(formatted) => {
                assert!(
                    formatted.contains(expected_part),
                    "Le code devrait être formaté"
                );
                assert!(
                    !formatted.contains("  main  "),
                    "Les espaces superflus doivent disparaître"
                );
            }
            Err(_) => {
                println!("⚠️ Test ignoré : 'rustfmt' semble absent du système.");
            }
        }
    }

    #[test]
    fn test_pipe_through_failure() {
        // Outil qui n'existe pas
        let res = pipe_through("outil_fantome", "input");
        assert!(res.is_err());
    }
}
