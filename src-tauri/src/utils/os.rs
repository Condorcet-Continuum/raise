// FICHIER : src-tauri/src/utils/os.rs

use crate::raise_error; // NOUVEAU : Import de la fondation RAISE
use crate::utils::RaiseResult;
use std::io::{self, BufRead, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use tracing::{debug, instrument, warn};

/// Exécute une commande système et capture sa sortie.
/// Utile pour lancer des outils comme Cargo, Git, etc.
#[instrument(skip(args), fields(cmd = cmd, cwd = ?cwd))]
pub fn exec_command(cmd: &str, args: &[&str], cwd: Option<&Path>) -> RaiseResult<String> {
    debug!("🚀 Exécution commande système : {} {:?}", cmd, args);

    let mut command = Command::new(cmd);
    command.args(args);

    // Configuration du dossier courant
    if let Some(dir) = cwd {
        if !dir.exists() {
            raise_error!(
                "ERR_OS_CWD_NOT_FOUND",
                error = "Dossier d'exécution introuvable",
                context = serde_json::json!({ "path": dir.to_string_lossy() })
            );
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
                raise_error!(
                    "ERR_OS_COMMAND_EXIT_ERROR",
                    error = stderr.trim(),
                    context = serde_json::json!({
                        "cmd": cmd,
                        "args": args,
                        "exit_code": output.status.code()
                    })
                );
            }
        }
        Err(e) => {
            // Remplacement de fs_error par raise_error!
            raise_error!(
                "ERR_OS_EXEC_SPAWN",
                error = e,
                context = serde_json::json!({ "cmd": cmd, "args": args })
            );
        }
    }
}

/// Passe une chaîne de caractères dans l'entrée standard (stdin) d'une commande
/// et récupère le résultat transformé (stdout).
#[instrument(skip(input), fields(cmd = cmd))]
pub fn pipe_through(cmd: &str, input: &str) -> RaiseResult<String> {
    // 1. Lancement du processus
    let mut child = match Command::new(cmd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => raise_error!(
            "ERR_OS_PIPE_SPAWN",
            error = e,
            context = serde_json::json!({ "cmd": cmd })
        ),
    };

    // 2. Écriture dans stdin
    if let Some(mut stdin) = child.stdin.take() {
        if let Err(e) = stdin.write_all(input.as_bytes()) {
            raise_error!(
                "ERR_OS_PIPE_WRITE",
                error = e,
                context = serde_json::json!({ "cmd": cmd })
            );
        }
    }

    // 3. Attente du résultat
    let output = match child.wait_with_output() {
        Ok(o) => o,
        Err(e) => raise_error!(
            "ERR_OS_PIPE_WAIT",
            error = e,
            context = serde_json::json!({ "cmd": cmd })
        ),
    };

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        raise_error!(
            "ERR_OS_PIPE_EXEC_ERROR",
            error = stderr.trim(),
            context = serde_json::json!({ "cmd": cmd })
        );
    }
}

/// Force l'affichage immédiat sur la console (flush stdout).
#[instrument]
pub fn flush_stdout() -> RaiseResult<()> {
    if let Err(e) = io::stdout().flush() {
        raise_error!("ERR_OS_STDOUT_FLUSH", error = e);
    }
    Ok(())
}

/// Lit une ligne depuis l'entrée standard (stdin) de manière synchrone.
#[instrument]
pub fn read_stdin_line() -> RaiseResult<String> {
    let mut input = String::new();
    let stdin = io::stdin();
    let mut handle = stdin.lock();

    match handle.read_line(&mut input) {
        Ok(_) => Ok(input.trim().to_string()),
        Err(e) => {
            raise_error!(
                "ERR_OS_STDIN_READ",
                error = e,
                context = serde_json::json!({ "source": "stdin" })
            );
        }
    }
}

/// Affiche un message et attend une saisie utilisateur sur la même ligne.
pub fn prompt(message: &str) -> RaiseResult<String> {
    print!("{}", message);
    flush_stdout()?;
    read_stdin_line()
}

// --- TESTS UNITAIRES ---
#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::prelude::*;

    #[test]
    fn test_exec_command_success() {
        let res = exec_command("cargo", &["--version"], None);
        assert!(res.is_ok());
    }

    #[test]
    fn test_exec_command_not_found() {
        let res = exec_command("commande_qui_n_existe_pas_12345", &[], None);

        // On s'assure que c'est bien une erreur
        assert!(res.is_err());

        // MIGRATION : On extrait directement les données puisque AppError::Structured est l'unique variant !
        let AppError::Structured(data) = res.unwrap_err();

        // On vérifie le code d'erreur structuré
        assert_eq!(data.code, "ERR_OS_EXEC_SPAWN");
    }

    #[test]
    fn test_exec_command_failure_status() {
        let res = exec_command("cargo", &["build", "--manifest-path", "ghost.toml"], None);

        // On s'assure que la commande a bien échoué
        assert!(res.is_err());

        // MIGRATION : On déballe directement l'unique variant de notre erreur
        let AppError::Structured(data) = res.unwrap_err();

        // On valide le code d'erreur attendu
        assert_eq!(data.code, "ERR_OS_COMMAND_EXIT_ERROR");
    }
}
