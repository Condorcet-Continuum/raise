use crate::utils::prelude::*;

#[derive(Debug, Serializable, Deserializable, Clone)]
pub struct TrafficStats {
    pub views: u64,
    pub unique_visitors: u64,
    pub timestamp: String,
}

pub struct GitTool;

impl GitTool {
    /// 🛠️ Commande Interne : Exécution via `AsyncCommand`.
    /// Utilise strictement `match` et la macro `raise_error!` sans `return Err` redondant.
    async fn execute_git(args: &[&str], cwd: &Path) -> RaiseResult<String> {
        let command_res = AsyncCommand::new("git")
            .args(args)
            .current_dir(cwd)
            .output()
            .await;

        match command_res {
            Ok(output) => {
                if output.status.success() {
                    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
                } else {
                    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                    // 🎯 La macro raise_error! gère déjà le retour Err(...)
                    raise_error!(
                        "ERR_GIT_COMMAND_FAILED",
                        error = stderr,
                        context = json_value!({ "args": args, "exit_code": output.status.code() })
                    )
                }
            }
            Err(e) => raise_error!(
                "ERR_GIT_PROCESS_SPAWN",
                error = e,
                context = json_value!({ "args": args, "path": cwd.to_string_lossy() })
            ),
        }
    }

    /// 📊 Récupération souveraine des statistiques de trafic (GitHub API).
    pub async fn fetch_traffic(owner: &str, repo: &str, token: &str) -> RaiseResult<TrafficStats> {
        user_info!("INF_GIT_FETCH_TRAFFIC", json_value!({ "repo": repo }));

        // 1. 🎯 Utilisation du Singleton HTTP via la façade (Zéro Dette)
        // Le timeout (60s) et le User-Agent ("Raise-Core/...") sont hérités automatiquement.
        let client = get_client();
        let url = format!(
            "https://api.github.com/repos/{}/{}/traffic/views",
            owner, repo
        );

        // 2. Envoi de la requête
        let response_res = client
            .get(&url)
            .header("Authorization", format!("Bearer {}", token))
            .header("Accept", "application/vnd.github+json")
            .send()
            .await;

        let response = match response_res {
            Ok(res) => res,
            Err(e) => raise_error!(
                "ERR_GIT_NETWORK_FAILURE",
                error = e,
                context = json_value!({ "url": url })
            ),
        };

        // 3. Validation du statut HTTP
        if !response.status().is_success() {
            let status = response.status();
            raise_error!(
                "ERR_GIT_API_RESPONSE",
                error = format!("Status: {}", status),
                context = json_value!({ "url": url, "status": status.as_u16() })
            )
        }

        // 4. Désérialisation du JSON
        let body_res: Result<JsonValue, _> = response.json().await;
        let body = match body_res {
            Ok(json) => json,
            Err(e) => raise_error!("ERR_GIT_JSON_DECODING", error = e),
        };

        Ok(TrafficStats {
            views: body["count"].as_u64().unwrap_or(0),
            unique_visitors: body["uniques"].as_u64().unwrap_or(0),
            timestamp: UtcClock::now().to_rfc3339(),
        })
    }

    /// 🔒 Publication Sécurisée : Cycle Add -> Commit -> Push.
    pub async fn secure_publish(cwd: &Path, message: &str) -> RaiseResult<String> {
        user_info!(
            "INF_GIT_PUBLISH_START",
            json_value!({ "path": cwd.to_string_lossy() })
        );

        // 1. Stage
        match Self::execute_git(&["add", "."], cwd).await {
            Ok(_) => (),
            Err(e) => return Err(e),
        };

        // 2. Commit déterministe avec ID unique Raise
        let xai_id = UniqueId::new_v4();
        let full_msg = format!("ai(core): {} [XAI-Ref: {}]", message, xai_id);

        match Self::execute_git(&["commit", "-m", &full_msg], cwd).await {
            Ok(_) => (),
            Err(e) => return Err(e),
        };

        // 3. Push
        match Self::execute_git(&["push"], cwd).await {
            Ok(stdout) => {
                user_success!(
                    "SUC_GIT_PUBLISH_COMPLETE",
                    json_value!({ "commit_id": xai_id.to_string() })
                );
                Ok(stdout)
            }
            Err(e) => Err(e),
        }
    }
}

// =========================================================================
// TESTS UNITAIRES (VALIDATION GPU & VRAM)
// =========================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::testing::mock::AgentDbSandbox;

    #[async_test]
    #[serial_test::serial] // Sécurité : L'orchestrateur charge l'IA
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_git_tool_execution_flow() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await?;
        let root = &sandbox.domain_root;

        match GitTool::execute_git(&["--version"], root).await {
            Ok(version) => assert!(version.contains("git version")),
            Err(e) => panic!("Échec innatendu de GitTool: {:?}", e),
        }
        Ok(())
    }

    #[async_test]
    #[serial_test::serial] // Sécurité : L'orchestrateur charge l'IA
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_git_publish_error_on_invalid_repo() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await?;
        let invalid_path = sandbox.domain_root.join("not_a_repo");

        // Création asynchrone via la façade
        match fs::ensure_dir_async(&invalid_path).await {
            Ok(_) => (),
            Err(e) => panic!("Erreur lors de la création du dossier: {:?}", e),
        };

        match GitTool::secure_publish(&invalid_path, "Test Fail").await {
            Ok(_) => panic!("Le test aurait dû échouer"),
            Err(e) => {
                let err_msg = format!("{:?}", e);
                assert!(err_msg.contains("ERR_GIT_COMMAND_FAILED"));
            }
        }

        Ok(())
    }
}
