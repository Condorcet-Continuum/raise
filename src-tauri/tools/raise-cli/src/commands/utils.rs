// FICHIER : src-tauri/tools/raise-cli/src/commands/utils.rs

use clap::{Args, Subcommand};
use raise::json_db::collections::manager::CollectionsManager;
use raise::{
    user_info, user_success, user_warn,
    utils::{
        config::AppConfig,
        io::{self},
        prelude::*,
    },
};

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
        username: String,
    },
    /// Ferme la session actuelle et nettoie les données
    Logout,
}

pub async fn handle(args: UtilsArgs, ctx: CliContext) -> RaiseResult<()> {
    // Heartbeat automatique pour signaler l'activité à chaque appel
    let _ = ctx.session_mgr.touch().await;

    match args.command {
        UtilsCommands::Info => {
            println!("--- 🛠️ RAISE SYSTEM INFO ---");

            // 1. STATUT DE LA SESSION (Via le contexte)
            if let Some(session) = ctx.session_mgr.get_current_session().await {
                user_info!(
                    "CLI_SESSION_ACTIVE",
                    json!({
                        "user_id": session.user_id,
                        "status": format!("{:?}", session.status),
                        "session_id": session.id,
                        "domain": session.context.current_domain,
                        "db": session.context.current_db,
                    })
                );
            } else {
                user_warn!(
                    "CLI_SESSION_INACTIVE",
                    json!({"hint": "Aucune session n'est détectée."})
                );
            }

            // 2. VERSIONS ET ENVIRONNEMENT
            user_info!(
                "APP_VERSION",
                json!({
                    "version": env!("CARGO_PKG_VERSION"),
                    "env": if cfg!(debug_assertions) { "development" } else { "production" }
                })
            );

            // 3. VÉRIFICATION DU MOTEUR LLM
            let mut provider = String::from("Non configuré");
            let mut model = String::from("Inconnu");

            // Utilisation du storage déjà ouvert dans le contexte
            let manager = CollectionsManager::new(
                &ctx.storage,
                &ctx.config.system_domain,
                &ctx.config.system_db,
            );

            if let Ok(settings) = AppConfig::get_component_settings(&manager, "llm").await {
                provider = settings
                    .get("provider")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Local")
                    .to_string();
                model = settings
                    .get("rust_model_file")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Inconnu")
                    .to_string();
            }

            user_info!(
                "LLM_ENGINE_STATUS",
                json!({
                    "provider": provider,
                    "model": model,
                    "is_active": provider != "Non configuré"
                })
            );

            // 4. VÉRIFICATION SYSTÈME DE FICHIERS
            if let Some(path) = ctx.config.get_path("PATH_RAISE_DOMAIN") {
                let exists = io::exists(&path).await;
                user_info!("FS_CHECK", json!({ "path": path, "exists": exists }));
            }
        }

        UtilsCommands::Ping => {
            user_success!("PONG", json!({"status": "alive", "timestamp": Utc::now()}));
        }

        UtilsCommands::Whoami => {
            // 🎯 Utilisation de get_current_session()
            match ctx.session_mgr.get_current_session().await {
                Some(session) => {
                    user_info!(
                        "CURRENT_USER",
                        json!({
                            "username": session.user_id,
                            "session_id": session.id,
                            "created_at": session.created_at,
                            "last_activity": session.last_activity_at
                        })
                    );
                }
                None => {
                    user_warn!(
                        "NO_ACTIVE_SESSION",
                        json!({"hint": "Utilisez 'utils login <username>' pour vous connecter."})
                    );
                }
            }
        }

        UtilsCommands::Login { username } => {
            // 🎯 Utilisation de start_session() qui gère mémoire + DB
            user_info!("AUTH_START", json!({ "target_user": username }));

            let session = ctx.session_mgr.start_session(&username).await?;

            user_success!(
                "AUTH_SUCCESS",
                json!({
                    "user": session.user_id,
                    "session_id": session.id,
                    "message": "Session manuelle établie et persistée."
                })
            );
        }

        UtilsCommands::Logout => {
            // 🎯 Utilisation de end_session() pour le nettoyage complet
            if let Some(session) = ctx.session_mgr.get_current_session().await {
                let user_id = session.user_id.clone();
                ctx.session_mgr.end_session().await?;

                user_success!(
                    "AUTH_LOGOUT",
                    json!({
                        "previous_user": user_id,
                        "status": "disconnected",
                        "cleanup": "success"
                    })
                );
            } else {
                user_warn!(
                    "LOGOUT_SKIPPED",
                    json!({"reason": "No active session to terminate"})
                );
            }
        }
    }
    Ok(())
}

// =========================================================================
// TESTS UNITAIRES ET D'INTÉGRATION CLI
// =========================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use crate::CliContext;
    use raise::utils::mock::DbSandbox;
    use raise::utils::session::SessionManager;
    use raise::utils::Arc;

    /// Teste le cycle de vie complet d'une session manuelle
    #[tokio::test]
    async fn test_session_full_lifecycle() {
        let sandbox = DbSandbox::new().await;
        let storage = Arc::new(sandbox.storage.clone());
        let session_mgr = SessionManager::new(storage.clone());

        let ctx = CliContext {
            config: AppConfig::get(),
            session_mgr: session_mgr.clone(),
            storage,
        };

        // 1. État initial (Pas de session)
        let who_args = UtilsArgs {
            command: UtilsCommands::Whoami,
        };
        handle(who_args, ctx.clone()).await.unwrap();
        assert!(session_mgr.get_current_session().await.is_none());

        // 🎯 FIX: Utilisation d'un UUID valide au lieu de "manual_tester"
        let test_uuid = "11111111-1111-1111-1111-111111111111";

        // 2. Login
        let login_args = UtilsArgs {
            command: UtilsCommands::Login {
                username: test_uuid.into(),
            },
        };
        handle(login_args, ctx.clone()).await.unwrap();

        let s = session_mgr
            .get_current_session()
            .await
            .expect("La session devrait exister");
        assert_eq!(s.user_id, test_uuid);

        // 3. Logout
        let logout_args = UtilsArgs {
            command: UtilsCommands::Logout,
        };
        handle(logout_args, ctx.clone()).await.unwrap();
        assert!(
            session_mgr.get_current_session().await.is_none(),
            "La session devrait être supprimée après logout"
        );
    }

    /// Teste la robustesse de la commande Logout quand aucune session n'est active
    #[tokio::test]
    async fn test_logout_without_session() {
        let sandbox = DbSandbox::new().await;
        let storage = Arc::new(sandbox.storage.clone());
        let ctx = CliContext {
            config: AppConfig::get(),
            session_mgr: SessionManager::new(storage.clone()),
            storage,
        };

        let args = UtilsArgs {
            command: UtilsCommands::Logout,
        };
        // Ne doit pas retourner d'erreur (doit être idempotent)
        assert!(handle(args, ctx).await.is_ok());
    }

    /// Teste le changement d'utilisateur (Login sur une session existante)
    #[tokio::test]
    async fn test_relogin_switches_user() {
        let sandbox = DbSandbox::new().await;
        let storage = Arc::new(sandbox.storage.clone());
        let session_mgr = SessionManager::new(storage.clone());
        let ctx = CliContext {
            config: AppConfig::get(),
            session_mgr: session_mgr.clone(),
            storage,
        };

        // 🎯 FIX: Utilisation d'UUIDs valides au lieu de "user_a" et "user_b"
        let user_a_uuid = "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa";
        let user_b_uuid = "bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb";

        // Login User A
        handle(
            UtilsArgs {
                command: UtilsCommands::Login {
                    username: user_a_uuid.into(),
                },
            },
            ctx.clone(),
        )
        .await
        .unwrap();

        // Login User B (Doit écraser la session en mémoire via start_session)
        handle(
            UtilsArgs {
                command: UtilsCommands::Login {
                    username: user_b_uuid.into(),
                },
            },
            ctx.clone(),
        )
        .await
        .unwrap();

        let current = session_mgr.get_current_session().await.unwrap();
        assert_eq!(current.user_id, user_b_uuid);
    }

    /// Teste la commande Info pour s'assurer qu'elle s'exécute sans paniquer
    #[tokio::test]
    async fn test_info_command_execution() {
        let sandbox = DbSandbox::new().await;
        let storage = Arc::new(sandbox.storage.clone());
        let ctx = CliContext {
            config: AppConfig::get(),
            session_mgr: SessionManager::new(storage.clone()),
            storage,
        };

        let args = UtilsArgs {
            command: UtilsCommands::Info,
        };
        assert!(handle(args, ctx).await.is_ok());
    }
}
