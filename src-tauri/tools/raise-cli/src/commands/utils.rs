// FICHIER : src-tauri/tools/raise-cli/src/commands/utils.rs

use clap::{Args, Subcommand};
use raise::json_db::collections::manager::CollectionsManager;
use raise::json_db::query::{Condition, FilterOperator, Query, QueryEngine, QueryFilter};
use raise::utils::prelude::*;

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
    // Heartbeat automatique pour signaler l'activité à chaque appel
    let _ = ctx.session_mgr.touch().await;

    match args.command {
        UtilsCommands::Info => {
            println!("--- 🛠️ RAISE SYSTEM INFO ---");

            // 1. STATUT DE LA SESSION (Via le contexte)
            if let Some(session) = ctx.session_mgr.get_current_session().await {
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
            } else {
                user_warn!(
                    "CLI_SESSION_INACTIVE",
                    json_value!({"hint": "Aucune session n'est détectée."})
                );
            }

            // 2. VERSIONS ET ENVIRONNEMENT
            user_info!(
                "APP_VERSION",
                json_value!({
                    "version": env!("CARGO_PKG_VERSION"),
                    "env": if cfg!(debug_assertions) { "development" } else { "production" }
                })
            );

            // 3. VÉRIFICATION DU MOTEUR LLM
            let mut provider = String::from("Non configuré");
            let mut model = String::from("Inconnu");

            // 🎯 On utilise directement les valeurs résolues du contexte !
            let manager = CollectionsManager::new(&ctx.storage, &ctx.active_domain, &ctx.active_db);

            if let Ok(settings) = AppConfig::get_component_settings(&manager, "llm").await {
                // `settings` est déjà un JsonValue, on peut le requêter directement
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
                json_value!({
                    "provider": provider,
                    "model": model,
                    "is_active": provider != "Non configuré",
                    "domain": ctx.active_domain, // 🎯 Traçabilité !
                    "db": ctx.active_db
                })
            );

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

        UtilsCommands::Whoami => {
            // 🎯 Utilisation de get_current_session()
            match ctx.session_mgr.get_current_session().await {
                Some(session) => {
                    user_info!(
                        "CURRENT_USER",
                        json_value!({
                            "userhandle": session.user_id,
                            "active_domain": ctx.active_domain,
                            "active_db": ctx.active_db,
                            "session_id": session.id,
                            "created_at": session.created_at,
                            "last_activity": session.last_activity_at
                        })
                    );
                }
                None => {
                    user_warn!(
                        "NO_ACTIVE_SESSION",
                        json_value!({"hint": "Utilisez 'utils login <userhandle>' pour vous connecter."})
                    );
                }
            }
        }

        UtilsCommands::Login { userhandle } => {
            // 🎯 Utilisation de start_session() qui gère mémoire + DB
            user_info!("AUTH_START", json_value!({ "target_user": userhandle }));

            let session = ctx.session_mgr.start_session(&userhandle).await?;

            user_success!(
                "AUTH_SUCCESS",
                json_value!({
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
                    json_value!({
                        "previous_user": user_id,
                        "status": "disconnected",
                        "cleanup": "success"
                    })
                );
            } else {
                user_warn!(
                    "LOGOUT_SKIPPED",
                    json_value!({"reason": "No active session to terminate"})
                );
            }
        }

        UtilsCommands::Config { action, key, value } => {
            match action.to_lowercase().as_str() {
                // 👁️ AFFICHER LA CONFIGURATION
                "show" => {
                    let session = ctx.session_mgr.get_current_session().await;
                    user_info!(
                        "CLI_CURRENT_CONFIG",
                        json_value!({
                            "context": {
                                "active_user": ctx.active_user,
                                "active_domain": ctx.active_domain,
                                "active_db": ctx.active_db,
                            },
                            "session": session,
                            "global_config": {
                                "system_domain": ctx.config.system_domain,
                                "system_db": ctx.config.system_db,
                                "env_mode": ctx.config.core.env_mode,
                                "language": ctx.config.core.language,
                                "log_level": ctx.config.core.log_level,
                            }
                        })
                    );
                }

                // ✏️ MODIFIER LA CONFIGURATION UTILISATEUR
                "set" => {
                    let Some(k) = key else {
                        user_warn!(
                            "CLI_USAGE",
                            json_value!({"hint": "Usage: utils config set <key> <value>"})
                        );
                        return Ok(());
                    };
                    let Some(v) = value else {
                        user_warn!(
                            "CLI_USAGE",
                            json_value!({"hint": "Usage: utils config set <key> <value>"})
                        );
                        return Ok(());
                    };

                    let sys_mgr = CollectionsManager::new(&ctx.storage, "_system", "_system");

                    // 1. Trouver le document de l'utilisateur courant
                    let mut query = Query::new("users");
                    query.filter = Some(QueryFilter {
                        operator: FilterOperator::And,
                        conditions: vec![Condition::eq("handle", json_value!(&ctx.active_user))],
                    });

                    let res = QueryEngine::new(&sys_mgr).execute_query(query).await?;

                    if let Some(doc) = res.documents.first() {
                        let id = doc.get("_id").and_then(|id| id.as_str()).unwrap_or("");

                        // 2. Création du patch JSON (gère un niveau d'imbrication max, ex: core.log_level)
                        let patch = if k.contains('.') {
                            let parts: Vec<&str> = k.split('.').collect();
                            let mut inner = raise::utils::data::json::JsonObject::new();
                            inner.insert(parts[1].to_string(), json_value!(v.clone()));

                            let mut outer = raise::utils::data::json::JsonObject::new();
                            outer.insert(parts[0].to_string(), JsonValue::Object(inner));
                            JsonValue::Object(outer)
                        } else {
                            let mut map = raise::utils::data::json::JsonObject::new();
                            map.insert(k.clone(), json_value!(v.clone()));
                            JsonValue::Object(map)
                        };

                        // 3. Mise à jour persistante dans JsonDB
                        match sys_mgr.update_document("users", id, patch).await {
                            Ok(_) => {
                                user_success!(
                                    "CONFIG_UPDATED",
                                    json_value!({
                                        "user": ctx.active_user,
                                        "key": k,
                                        "new_value": v,
                                        "hint": "Re-tapez 'login <votre_nom>' pour rafraîchir la session avec ces nouveaux paramètres !"
                                    })
                                );
                            }
                            Err(e) => {
                                user_error!(
                                    "CONFIG_UPDATE_FAILED",
                                    json_value!({"error": e.to_string()})
                                );
                            }
                        }
                    } else {
                        user_error!(
                            "USER_NOT_FOUND",
                            json_value!({
                                "userhandle": ctx.active_user,
                                "hint": "Impossible de modifier la configuration d'un utilisateur non persistant."
                            })
                        );
                    }
                }
                _ => {
                    user_warn!(
                        "CLI_USAGE",
                        json_value!({"hint": "Action inconnue. Usage: utils config [show|set]"})
                    );
                }
            }
        }

        // 🎯 CHANGER DE DOMAINE
        UtilsCommands::UseDomain { domain } => match ctx.session_mgr.switch_domain(&domain).await {
            Ok(new_ctx) => {
                user_success!("DOMAIN_SWITCHED", json_value!(new_ctx));
            }
            Err(e) => {
                user_error!("DOMAIN_ERROR", json_value!({"error": e.to_string()}));
            }
        },

        // 🎯 CHANGER DE BASE DE DONNÉES
        UtilsCommands::UseDb { db } => match ctx.session_mgr.switch_db(&db).await {
            Ok(new_ctx) => {
                user_success!("DB_SWITCHED", json_value!(new_ctx));
            }
            Err(e) => {
                user_error!("DB_ERROR", json_value!({"error": e.to_string()}));
            }
        },
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
    use raise::json_db::collections::manager::CollectionsManager;
    use raise::utils::context::SessionManager; // 🎯 Requis pour l'injection

    #[cfg(test)]
    use raise::utils::testing::mock::inject_mock_user;
    #[cfg(test)]
    use raise::utils::testing::DbSandbox; // 🎯 Import du helper magique

    /// Teste le cycle de vie complet d'une session manuelle
    #[async_test]
    async fn test_session_full_lifecycle() {
        let sandbox = DbSandbox::new().await;
        let storage = SharedRef::new(sandbox.storage.clone());
        let session_mgr = SessionManager::new(storage.clone());
        raise::json_db::jsonld::VocabularyRegistry::init_mock_for_tests();

        let ctx = CliContext::mock(AppConfig::get(), session_mgr.clone(), storage.clone());

        // 1. État initial (Pas de session)
        let who_args = UtilsArgs {
            command: UtilsCommands::Whoami,
        };
        handle(who_args, ctx.clone()).await.unwrap();
        assert!(session_mgr.get_current_session().await.is_none());

        let test_user = "Astra-CLI-Tester";

        // 🎯 INJECTION DE L'UTILISATEUR AVANT LE LOGIN
        let db_mgr = CollectionsManager::new(
            &sandbox.storage,
            &AppConfig::get().system_domain,
            &AppConfig::get().system_db,
        );
        inject_mock_user(&db_mgr, test_user).await;

        // 2. Login
        let login_args = UtilsArgs {
            command: UtilsCommands::Login {
                userhandle: test_user.into(),
            },
        };
        handle(login_args, ctx.clone()).await.unwrap();

        let s = session_mgr
            .get_current_session()
            .await
            .expect("La session devrait exister");
        assert_eq!(s.user_handle, test_user); // 🎯 On vérifie le nom de l'utilisateur

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
    #[async_test]
    async fn test_logout_without_session() {
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
        // Ne doit pas retourner d'erreur (doit être idempotent)
        assert!(handle(args, ctx).await.is_ok());
    }

    /// Teste le changement d'utilisateur (Login sur une session existante)
    #[async_test]
    async fn test_relogin_switches_user() {
        let sandbox = DbSandbox::new().await;
        let storage = SharedRef::new(sandbox.storage.clone());
        let session_mgr = SessionManager::new(storage.clone());
        raise::json_db::jsonld::VocabularyRegistry::init_mock_for_tests();
        let ctx = CliContext::mock(AppConfig::get(), session_mgr.clone(), storage);

        let user_a = "Agent-A";
        let user_b = "Agent-B";

        // 🎯 INJECTION DES DEUX UTILISATEURS
        let db_mgr = CollectionsManager::new(
            &sandbox.storage,
            &AppConfig::get().system_domain,
            &AppConfig::get().system_db,
        );
        inject_mock_user(&db_mgr, user_a).await;
        inject_mock_user(&db_mgr, user_b).await;

        // Login User A
        handle(
            UtilsArgs {
                command: UtilsCommands::Login {
                    userhandle: user_a.into(),
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
                    userhandle: user_b.into(),
                },
            },
            ctx.clone(),
        )
        .await
        .unwrap();

        let current = session_mgr.get_current_session().await.unwrap();
        assert_eq!(current.user_handle, user_b); // 🎯 On vérifie que B a pris le dessus
    }

    /// Teste la commande Info pour s'assurer qu'elle s'exécute sans paniquer
    #[async_test]
    async fn test_info_command_execution() {
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
        assert!(handle(args, ctx).await.is_ok());
    }
}
