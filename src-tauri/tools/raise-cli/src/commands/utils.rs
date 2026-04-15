// FICHIER : src-tauri/tools/raise-cli/src/commands/utils.rs

use clap::{Args, Subcommand};
use raise::json_db::collections::manager::CollectionsManager;
use raise::json_db::query::{Condition, FilterOperator, Query, QueryEngine, QueryFilter};
use raise::utils::prelude::*; // 🎯 Façade Unique RAISE

// 🎯 Import du contexte global CLI
use crate::CliContext;

/// Outils de maintenance et de gestion de session pour RAISE.
#[derive(Args, Clone, Debug)]
pub struct UtilsArgs {
    #[command(subcommand)]
    pub command: UtilsCommands,
}

#[derive(Subcommand, Clone, Debug)]
pub enum UtilsCommands {
    /// Affiche la configuration active, le statut session et les moteurs
    Info,
    /// Vérifie la connectivité interne (Ping)
    Ping,
    /// Affiche l'identité de l'utilisateur actuellement connecté
    Whoami,
    /// Se connecter avec un identifiant utilisateur (Force une nouvelle session)
    Login {
        /// Identifiant utilisateur (ex: "zair", "admin")
        userhandle: String,
    },
    /// Ferme la session actuelle et nettoie les données
    Logout,
    Config {
        /// L'action à effectuer ("show" ou "set")
        #[arg(default_value = "show")]
        action: String,
        /// La clé à modifier (ex: "core.log_level", "default_db")
        key: Option<String>,
        /// La nouvelle valeur
        value: Option<String>,
    },

    /// Change le domaine actif de la session (bascule automatiquement sur la DB par défaut du domaine)
    UseDomain {
        /// Le nom (handle) du domaine cible
        domain: String,
    },
    /// Change la base de données active (doit appartenir au domaine en cours)
    UseDb {
        /// Le nom (handle) de la base de données cible
        db: String,
    },
}

pub async fn handle(args: UtilsArgs, ctx: CliContext) -> RaiseResult<()> {
    // Heartbeat automatique pour signaler l'activité
    match ctx.session_mgr.touch().await {
        Ok(_) => user_debug!("SESSION_TOUCHED"),
        Err(e) => user_error!(
            "ERR_SESSION_HEARTBEAT",
            json_value!({"error": e.to_string()})
        ),
    }

    match args.command {
        UtilsCommands::Info => {
            println!("--- 🛠️ RAISE SYSTEM INFO ---");

            // 1. STATUT DE LA SESSION
            match ctx.session_mgr.get_current_session().await {
                Some(session) => {
                    user_info!(
                        "CLI_SESSION_ACTIVE",
                        json_value!({
                            "user_id": session.user_id,
                            "status": format!("{:?}", session.status),
                            "session_id": session.id,
                            "domain": session.context.current_domain,
                            "db": session.context.current_db,
                        })
                    );
                }
                None => user_warn!(
                    "CLI_SESSION_INACTIVE",
                    json_value!({"hint": "Aucune session n'est détectée."})
                ),
            }

            // 2. VERSIONS ET ENVIRONNEMENT
            user_info!(
                "APP_VERSION",
                json_value!({
                    "version": env!("CARGO_PKG_VERSION"),
                    "env": if cfg!(debug_assertions) { "development" } else { "production" }
                })
            );

            // 3. VÉRIFICATION DU MOTEUR LLM VIA MOUNT POINTS
            let manager = CollectionsManager::new(&ctx.storage, &ctx.active_domain, &ctx.active_db);
            match AppConfig::get_component_settings(&manager, "llm").await {
                Ok(settings) => {
                    user_info!(
                        "LLM_ENGINE_STATUS",
                        json_value!({
                            "provider": settings.get("provider").and_then(|v| v.as_str()).unwrap_or("Local"),
                            "model": settings.get("rust_model_file").and_then(|v| v.as_str()).unwrap_or("Inconnu"),
                            "is_active": true,
                            "domain": ctx.active_domain
                        })
                    );
                }
                Err(_) => user_warn!(
                    "LLM_ENGINE_OFFLINE",
                    json_value!({"hint": "Composant LLM non configuré"})
                ),
            }

            // 4. VÉRIFICATION SYSTÈME DE FICHIERS
            if let Some(path) = ctx.config.get_path("PATH_RAISE_DOMAIN") {
                let exists = fs::exists_async(&path).await;
                user_info!("FS_CHECK", json_value!({ "path": path, "exists": exists }));
            }
        }

        UtilsCommands::Ping => {
            user_success!(
                "PONG",
                json_value!({"status": "alive", "timestamp": UtcClock::now()})
            );
        }

        UtilsCommands::Whoami => match ctx.session_mgr.get_current_session().await {
            Some(session) => {
                user_info!(
                    "CURRENT_USER",
                    json_value!({
                        "userhandle": session.user_id,
                        "active_domain": ctx.active_domain,
                        "active_db": ctx.active_db,
                        "session_id": session.id
                    })
                );
            }
            None => {
                user_warn!(
                    "NO_ACTIVE_SESSION",
                    json_value!({"hint": "Utilisez 'utils login <userhandle>' pour vous connecter."})
                );
            }
        },

        UtilsCommands::Login { userhandle } => {
            user_info!("AUTH_START", json_value!({ "target_user": userhandle }));
            match ctx.session_mgr.start_session(&userhandle).await {
                Ok(session) => user_success!(
                    "AUTH_SUCCESS",
                    json_value!({"user": session.user_id, "session_id": session.id})
                ),
                Err(e) => raise_error!("ERR_AUTH_FAILED", error = e.to_string()),
            }
        }

        UtilsCommands::Logout => match ctx.session_mgr.get_current_session().await {
            Some(_) => match ctx.session_mgr.end_session().await {
                Ok(_) => user_success!("AUTH_LOGOUT", json_value!({"status": "disconnected"})),
                Err(e) => raise_error!("ERR_LOGOUT_FAIL", error = e.to_string()),
            },
            None => user_warn!(
                "LOGOUT_SKIPPED",
                json_value!({"reason": "No active session to terminate"})
            ),
        },

        UtilsCommands::Config { action, key, value } => {
            match action.to_lowercase().as_str() {
                "show" => {
                    let session = ctx.session_mgr.get_current_session().await;
                    user_info!(
                        "CLI_CURRENT_CONFIG",
                        json_value!({
                            "context": { "user": ctx.active_user, "domain": ctx.active_domain },
                            "session": session,
                            "mount_points": {
                                "system_domain": ctx.config.mount_points.system.domain,
                                "system_db": ctx.config.mount_points.system.db
                            }
                        })
                    );
                }
                "set" => {
                    let (k, v) = match (key, value) {
                        (Some(k), Some(v)) => (k, v),
                        _ => {
                            user_warn!(
                                "CLI_USAGE",
                                json_value!({"hint": "Usage: utils config set <key> <value>"})
                            );
                            return Ok(());
                        }
                    };

                    // Utilisation des Mount Points Système pour la gestion utilisateur
                    let sys_mgr = CollectionsManager::new(
                        &ctx.storage,
                        &ctx.config.mount_points.system.domain,
                        &ctx.config.mount_points.system.db,
                    );

                    let mut query = Query::new("users");
                    query.filter = Some(QueryFilter {
                        operator: FilterOperator::And,
                        conditions: vec![Condition::eq("handle", json_value!(&ctx.active_user))],
                    });

                    let engine = QueryEngine::new(&sys_mgr);
                    match engine.execute_query(query).await {
                        Ok(res) => {
                            if let Some(doc) = res.documents.first() {
                                let id = doc.get("_id").and_then(|id| id.as_str()).unwrap_or("");
                                let patch = json_value!({ k.clone(): v });
                                match sys_mgr.update_document("users", id, patch).await {
                                    Ok(_) => user_success!(
                                        "CONFIG_UPDATED",
                                        json_value!({"key": k, "value": v})
                                    ),
                                    Err(e) => raise_error!(
                                        "ERR_CONFIG_PERSIST_FAIL",
                                        error = e.to_string()
                                    ),
                                }
                            } else {
                                user_error!(
                                    "USER_NOT_FOUND",
                                    json_value!({"user": ctx.active_user})
                                );
                            }
                        }
                        Err(e) => raise_error!("ERR_DB_QUERY_FAIL", error = e.to_string()),
                    }
                }
                _ => user_warn!("CLI_USAGE", json_value!({"hint": "Actions: show | set"})),
            }
        }

        UtilsCommands::UseDomain { domain } => match ctx.session_mgr.switch_domain(&domain).await {
            Ok(new_ctx) => user_success!("DOMAIN_SWITCHED", json_value!(new_ctx)),
            Err(e) => user_error!("DOMAIN_ERROR", json_value!({"error": e.to_string()})),
        },

        UtilsCommands::UseDb { db } => match ctx.session_mgr.switch_db(&db).await {
            Ok(new_ctx) => user_success!("DB_SWITCHED", json_value!(new_ctx)),
            Err(e) => user_error!("DB_ERROR", json_value!({"error": e.to_string()})),
        },
    }
    Ok(())
}

// =========================================================================
// TESTS UNITAIRES ET D'INTÉGRATION CLI (Strictement respectés "Zéro Dette")
// =========================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use crate::CliContext;
    use raise::utils::context::SessionManager;
    use raise::utils::testing::mock::inject_mock_user;
    use raise::utils::testing::DbSandbox;

    #[async_test]
    async fn test_session_full_lifecycle() -> RaiseResult<()> {
        let sandbox = DbSandbox::new().await;
        let storage = SharedRef::new(sandbox.storage.clone());
        let session_mgr = SessionManager::new(storage.clone());
        raise::json_db::jsonld::VocabularyRegistry::init_mock_for_tests();

        let ctx = CliContext::mock(AppConfig::get(), session_mgr.clone(), storage.clone());

        // 1. État initial (Pas de session)
        let who_args = UtilsArgs {
            command: UtilsCommands::Whoami,
        };
        handle(who_args, ctx.clone()).await?;

        if session_mgr.get_current_session().await.is_some() {
            raise_error!(
                "ERR_TEST_ASSERTION_FAILED",
                error = "Une session ne devrait pas exister"
            );
        }

        let test_user = "Astra-CLI-Tester";
        let db_mgr = CollectionsManager::new(
            &sandbox.storage,
            &sandbox.config.mount_points.system.domain,
            &sandbox.config.mount_points.system.db,
        );
        inject_mock_user(&db_mgr, test_user).await;

        // 2. Login
        let login_args = UtilsArgs {
            command: UtilsCommands::Login {
                userhandle: test_user.into(),
            },
        };
        handle(login_args, ctx.clone()).await?;

        let s = match session_mgr.get_current_session().await {
            Some(session) => session,
            None => raise_error!(
                "ERR_TEST_ASSERTION_FAILED",
                error = "Échec de création de la session"
            ),
        };

        if s.user_handle != test_user {
            raise_error!(
                "ERR_TEST_ASSERTION_FAILED",
                error = "Le handle de session ne correspond pas"
            );
        }

        // 3. Logout
        let logout_args = UtilsArgs {
            command: UtilsCommands::Logout,
        };
        handle(logout_args, ctx.clone()).await?;

        if session_mgr.get_current_session().await.is_some() {
            raise_error!(
                "ERR_TEST_ASSERTION_FAILED",
                error = "La session devrait être vide après le logout"
            );
        }

        Ok(())
    }

    #[async_test]
    async fn test_logout_without_session() -> RaiseResult<()> {
        let sandbox = DbSandbox::new().await;
        let storage = SharedRef::new(sandbox.storage.clone());
        let ctx = CliContext::mock(
            AppConfig::get(),
            SessionManager::new(storage.clone()),
            storage,
        );
        let args = UtilsArgs {
            command: UtilsCommands::Logout,
        };

        match handle(args, ctx).await {
            Ok(_) => Ok(()),
            Err(e) => raise_error!("ERR_TEST_LOGOUT", error = e.to_string()),
        }
    }

    #[async_test]
    async fn test_relogin_switches_user() -> RaiseResult<()> {
        let sandbox = DbSandbox::new().await;
        let storage = SharedRef::new(sandbox.storage.clone());
        let session_mgr = SessionManager::new(storage.clone());
        raise::json_db::jsonld::VocabularyRegistry::init_mock_for_tests();
        let ctx = CliContext::mock(AppConfig::get(), session_mgr.clone(), storage);

        let user_a = "Agent-A";
        let user_b = "Agent-B";

        let db_mgr = CollectionsManager::new(
            &sandbox.storage,
            &sandbox.config.mount_points.system.domain,
            &sandbox.config.mount_points.system.db,
        );
        inject_mock_user(&db_mgr, user_a).await;
        inject_mock_user(&db_mgr, user_b).await;

        handle(
            UtilsArgs {
                command: UtilsCommands::Login {
                    userhandle: user_a.into(),
                },
            },
            ctx.clone(),
        )
        .await?;

        handle(
            UtilsArgs {
                command: UtilsCommands::Login {
                    userhandle: user_b.into(),
                },
            },
            ctx.clone(),
        )
        .await?;

        let current = match session_mgr.get_current_session().await {
            Some(session) => session,
            None => raise_error!(
                "ERR_TEST_ASSERTION_FAILED",
                error = "Aucune session active trouvée"
            ),
        };

        if current.user_handle != user_b {
            raise_error!(
                "ERR_TEST_ASSERTION_FAILED",
                error = "La session n'a pas basculé sur le nouvel utilisateur"
            );
        }

        Ok(())
    }

    #[async_test]
    async fn test_info_command_execution() -> RaiseResult<()> {
        let sandbox = DbSandbox::new().await;
        let storage = SharedRef::new(sandbox.storage.clone());
        raise::json_db::jsonld::VocabularyRegistry::init_mock_for_tests();
        let ctx = CliContext::mock(
            AppConfig::get(),
            SessionManager::new(storage.clone()),
            storage,
        );
        let args = UtilsArgs {
            command: UtilsCommands::Info,
        };

        match handle(args, ctx).await {
            Ok(_) => Ok(()),
            Err(e) => raise_error!("ERR_TEST_INFO", error = e.to_string()),
        }
    }

    /// 🎯 NOUVEAU TEST : Résilience de la partition système
    #[async_test]
    async fn test_utils_mount_point_integrity() -> RaiseResult<()> {
        let sandbox = DbSandbox::new().await;

        if sandbox.config.mount_points.system.domain.is_empty() {
            raise_error!(
                "ERR_TEST_ASSERTION_FAILED",
                error = "Le domaine du point de montage système est vide"
            );
        }

        if sandbox.config.mount_points.system.db.is_empty() {
            raise_error!(
                "ERR_TEST_ASSERTION_FAILED",
                error = "La base de données du point de montage système est vide"
            );
        }

        Ok(())
    }
}
