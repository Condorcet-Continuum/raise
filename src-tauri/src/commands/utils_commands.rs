// FICHIER : src-tauri/src/commands/utils_commands.rs

use crate::utils::{context, prelude::*};

use tauri::command;

/// Structure de réponse renvoyée au Frontend
#[derive(Debug, Serializable)]
pub struct SystemInfoResponse {
    pub app_version: String,
    pub env_mode: String,
    pub api_status: String,
    pub database_path: String,
}

/// Commande Tauri : Récupère les informations système
#[command]
pub async fn get_app_info() -> RaiseResult<SystemInfoResponse> {
    tracing::info!("📥 Commande reçue : get_app_info");

    let config = AppConfig::get();

    let raise_domain_path = config
        .get_path("PATH_RAISE_DOMAIN")
        .expect("ERREUR: PATH_RAISE_DOMAIN est introuvable dans la configuration !")
        .to_string_lossy()
        .to_string();

    let response = SystemInfoResponse {
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        env_mode: config.core.env_mode.clone(),
        api_status: "Connecté en local".to_string(),
        database_path: raise_domain_path,
    };

    tracing::debug!("✅ Réponse envoyée : {:?}", response);
    Ok(response)
}

// ============================================================================
// COMMANDES DE GESTION DE SESSION (FRONTEND)
// ============================================================================

/// Démarre une session pour un utilisateur donné
#[command]
pub async fn session_login(
    user_id: String,
    state: tauri::State<'_, context::SessionManager>,
) -> RaiseResult<context::Session> {
    tracing::info!(
        "📥 Commande reçue : session_login pour l'utilisateur '{}'",
        user_id
    );

    // Le SessionManager gère la création en mémoire ET la persistance dans json_db
    let session = state.start_session(&user_id).await?;

    tracing::info!("✅ Session démarrée : {}", session._id);
    Ok(session)
}

/// Clôture la session courante
#[command]
pub async fn session_logout(state: tauri::State<'_, context::SessionManager>) -> RaiseResult<()> {
    tracing::info!("📥 Commande reçue : session_logout");

    state.end_session().await?;

    tracing::info!("✅ Session clôturée avec succès");
    Ok(())
}

/// Récupère la session active (et met à jour le heartbeat)
#[command]
pub async fn session_get(
    state: tauri::State<'_, context::SessionManager>,
) -> RaiseResult<Option<context::Session>> {
    // On ne met qu'un log debug ici car le frontend risque d'appeler cette route très souvent
    tracing::debug!("📥 Commande reçue : session_get");

    let session = state.get_current_session().await;

    // Si une session existe, on considère cet appel comme une "activité" pour repousser l'expiration
    if session.is_some() {
        let _ = state.touch().await;
    }

    Ok(session)
}
