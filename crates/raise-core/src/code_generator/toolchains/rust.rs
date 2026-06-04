// FICHIER : src-tauri/src/code_generator/toolchains/rust.rs

use super::ToolchainStrategy;
use crate::utils::prelude::*;
use async_trait::async_trait;

// 🔒 Verrou global pour sérialiser les appels à Cargo et éviter les conflits de lock
// 🎯 Utilisation stricte de la façade RAISE (StaticCell + AsyncMutex)
static CARGO_LOCK: StaticCell<AsyncMutex<()>> = StaticCell::new();

fn cargo_lock() -> &'static AsyncMutex<()> {
    CARGO_LOCK.get_or_init(|| AsyncMutex::new(()))
}

pub struct RustToolchain;

impl RustToolchain {
    /// 🧠 Fonction pure : Extrait les messages d'erreur du flux JSON de Cargo Check
    fn parse_cargo_check_errors(output: &str) -> Vec<String> {
        let mut error_messages = Vec::new();
        for line in output.lines() {
            let trimmed_line = line.trim_start();
            if trimmed_line.starts_with('{') {
                if let Ok(json_line) = json::deserialize_from_str::<JsonValue>(trimmed_line) {
                    if json_line.get("reason").and_then(|v| v.as_str()) == Some("compiler-message")
                    {
                        if let Some(msg) = json_line.get("message") {
                            // On ignore les warnings, on ne garde que les erreurs strictes
                            if msg.get("level").and_then(|v| v.as_str()) == Some("error") {
                                if let Some(r) = msg.get("rendered").and_then(|v| v.as_str()) {
                                    error_messages.push(r.to_string());
                                }
                            }
                        }
                    }
                }
            }
        }
        error_messages
    }

    /// 🧠 Fonction pure : Extrait le chemin de l'exécutable WASM compilé
    fn parse_wasm_executable_path(output: &str) -> Option<String> {
        for line in output.lines() {
            let trimmed_line = line.trim_start();
            if trimmed_line.starts_with('{') {
                if let Ok(json_line) = json::deserialize_from_str::<JsonValue>(trimmed_line) {
                    if json_line.get("reason").and_then(|v| v.as_str()) == Some("compiler-artifact")
                    {
                        if let Some(target) = json_line.get("target") {
                            if target.get("test").and_then(|v| v.as_bool()) == Some(true) {
                                if let Some(exec) =
                                    json_line.get("executable").and_then(|v| v.as_str())
                                {
                                    return Some(exec.to_string());
                                }
                            }
                        }
                    }
                }
            }
        }
        None
    }
}

#[async_trait]
impl ToolchainStrategy for RustToolchain {
    async fn format(&self, path: &Path) -> RaiseResult<()> {
        match os::exec_command_async("rustfmt", &[path.to_string_lossy().as_ref()], None).await {
            Ok(_) => Ok(()),
            Err(e) => raise_error!(
                "ERR_CODEGEN_FORMAT_FAILED",
                error = e,
                context = json_value!({ "path": path.to_string_lossy() })
            ),
        }
    }

    async fn check(&self, _module_name: &str, cwd: Option<&Path>) -> RaiseResult<()> {
        // 🚦 Point de synchronisation via la façade
        let _guard = cargo_lock().lock().await;

        let output = match os::exec_command_async(
            "cargo",
            &["check", "--lib", "--message-format=json"],
            cwd,
        )
        .await
        {
            Ok(out) => out,
            Err(e) => raise_error!(
                "ERR_CODEGEN_CHECK_FAILED",
                error = e,
                context = json_value!({ "action": "cargo check" })
            ),
        };

        let error_messages = Self::parse_cargo_check_errors(&output);

        if !error_messages.is_empty() {
            raise_error!(
                "ERR_CODEGEN_COMPILATION_FAILED",
                context = json_value!({ "xai_feedback": error_messages.join("\n") })
            );
        }

        Ok(())
    }

    async fn test(&self, module_name: &str, cwd: Option<&Path>) -> RaiseResult<()> {
        // 🚦 Point de synchronisation via la façade
        let _guard = cargo_lock().lock().await;

        // 1. COMPILATION WASI (Sandboxing)
        let build_output = match os::exec_command_async(
            "cargo",
            &[
                "test",
                "--lib",
                module_name,
                "--target",
                "wasm32-wasip1",
                "--no-run",
                "--message-format=json",
            ],
            cwd,
        )
        .await
        {
            Ok(out) => out,
            Err(e) => raise_error!(
                "ERR_CODEGEN_WASM_BUILD_FAILED",
                error = e,
                context = json_value!({ "module": module_name })
            ),
        };

        // 2. EXTRACTION DU BINAIRE WASM
        let wasm_path = match Self::parse_wasm_executable_path(&build_output) {
            Some(p) => p,
            None => raise_error!(
                "ERR_CODEGEN_WASM_BIN_NOT_FOUND",
                context = json_value!({
                    "module": module_name,
                    "hint": "Cargo n'a pas produit d'exécutable WebAssembly. Vérifiez l'installation de la cible wasm32-wasip1."
                })
            ),
        };

        // 🔓 Relâchement explicite du verrou AVANT d'exécuter Wasmtime.
        drop(_guard);

        // 3. EXÉCUTION DANS LE RUNTIME (Zero-Trust)
        match os::exec_command_async("wasmtime", &["run", &wasm_path], cwd).await {
            Ok(_) => Ok(()),
            Err(e) => {
                let raw_error = e.to_string();
                let feedback = if let Some(idx) = raw_error.find("failures:") {
                    raw_error[idx..].to_string()
                } else {
                    raw_error
                };

                raise_error!(
                    "ERR_CODEGEN_TESTS_FAILED",
                    context = json_value!({
                        "xai_feedback": feedback,
                        "isolation_layer": "wasmtime"
                    })
                )
            }
        }
    }
}

// =========================================================================
// TESTS UNITAIRES
// =========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_cargo_check_no_errors() {
        let mock_output = r#"
{"reason":"compiler-artifact","package_id":"raise-core 0.1.0"}
{"reason":"build-finished","success":true}
        "#;

        let errors = RustToolchain::parse_cargo_check_errors(mock_output);
        assert!(errors.is_empty(), "Ne devrait détecter aucune erreur");
    }

    #[test]
    fn test_parse_cargo_check_with_errors_ignores_warnings() {
        let mock_output = r#"
{"reason":"compiler-message","message":{"level":"warning","rendered":"warning: unused variable `x`"}}
{"reason":"compiler-message","message":{"level":"error","rendered":"error[E0308]: mismatched types\nexpected `u32`, found `String`"}}
{"reason":"build-finished","success":false}
        "#;

        let errors = RustToolchain::parse_cargo_check_errors(mock_output);

        assert_eq!(
            errors.len(),
            1,
            "Doit ignorer le warning et capter l'erreur"
        );
        assert!(
            errors[0].contains("mismatched types"),
            "Doit extraire le message rendu"
        );
        assert!(
            !errors[0].contains("unused variable"),
            "Ne doit pas inclure le warning"
        );
    }

    #[test]
    fn test_parse_wasm_executable_path_found() {
        let mock_output = r#"
{"reason":"compiler-artifact","target":{"kind":["lib"],"test":false},"executable":null}
{"reason":"compiler-artifact","target":{"kind":["test"],"name":"raise_core_tests","test":true},"executable":"/path/to/target/wasm32-wasip1/debug/deps/raise_core_tests-12345.wasm"}
{"reason":"build-finished","success":true}
        "#;

        let path = RustToolchain::parse_wasm_executable_path(mock_output);
        assert_eq!(
            path,
            Some(
                "/path/to/target/wasm32-wasip1/debug/deps/raise_core_tests-12345.wasm".to_string()
            )
        );
    }

    #[test]
    fn test_parse_wasm_executable_path_not_found() {
        let mock_output = r#"
{"reason":"compiler-message","message":{"level":"error","rendered":"Syntax error"}}
{"reason":"build-finished","success":false}
        "#;

        let path = RustToolchain::parse_wasm_executable_path(mock_output);
        assert!(
            path.is_none(),
            "Ne doit rien retourner si l'artefact n'est pas présent"
        );
    }

    #[test]
    fn test_resilience_to_malformed_json() {
        let mock_output = r#"
    Compiling raise-core v0.1.0
    {"reason":"compiler-artifact","target":{"kind":["test"],"test":true},"executable":"/valid/path.wasm"}
    Finished test [unoptimized + debuginfo] target(s) in 2.00s
        "#;

        let path = RustToolchain::parse_wasm_executable_path(mock_output);
        assert_eq!(
            path,
            Some("/valid/path.wasm".to_string()),
            "Doit ignorer les lignes non-JSON et trouver le bon chemin"
        );
    }
}
