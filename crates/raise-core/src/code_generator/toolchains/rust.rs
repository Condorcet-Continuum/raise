// FICHIER : src-tauri/src/code_generator/toolchains/rust.rs

use super::ToolchainStrategy;
use crate::utils::prelude::*;
use async_trait::async_trait;

pub struct RustToolchain;

#[async_trait]
impl ToolchainStrategy for RustToolchain {
    async fn format(&self, path: &Path) -> RaiseResult<()> {
        os::exec_command_async("rustfmt", &[path.to_string_lossy().as_ref()], None).await?;
        Ok(())
    }

    async fn check(&self, _module_name: &str, cwd: Option<&Path>) -> RaiseResult<()> {
        let output =
            os::exec_command_async("cargo", &["check", "--lib", "--message-format=json"], cwd)
                .await?;

        let mut error_messages = Vec::new();
        for line in output.lines() {
            if line.starts_with('{') {
                if let Ok(json_line) = json::deserialize_from_str::<JsonValue>(line) {
                    if json_line.get("reason").and_then(|v| v.as_str()) == Some("compiler-message")
                    {
                        if let Some(msg) = json_line.get("message") {
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

        if !error_messages.is_empty() {
            raise_error!(
                "ERR_CODEGEN_COMPILATION_FAILED",
                context = json_value!({ "xai_feedback": error_messages.join("\n") })
            );
        }
        Ok(())
    }

    async fn test(&self, _module_name: &str, cwd: Option<&Path>) -> RaiseResult<()> {
        match os::exec_command_async("cargo", &["test", "--lib"], cwd).await {
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
                    context = json_value!({ "xai_feedback": feedback })
                );
            }
        }
    }
}
