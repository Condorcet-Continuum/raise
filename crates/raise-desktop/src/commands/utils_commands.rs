// FICHIER : crates/raise-desktop/src/commands/utils_commands.rs

use raise_core::utils::context::{Session, SessionManager};
use raise_core::utils::prelude::*;

// 🎯 On importe le service et les types de retour
use raise_core::services::utils_service::{self, SystemInfoResponse};

use tauri::{command, State};

#[command]
pub async fn get_app_info() -> RaiseResult<SystemInfoResponse> {
    utils_service::get_app_info().await
}

#[command]
pub async fn session_login(
    user_id: String,
    state: State<'_, SessionManager>,
) -> RaiseResult<Session> {
    utils_service::session_login(&user_id, state.inner()).await
}

#[command]
pub async fn session_logout(state: State<'_, SessionManager>) -> RaiseResult<()> {
    utils_service::session_logout(state.inner()).await
}

#[command]
pub async fn session_get(state: State<'_, SessionManager>) -> RaiseResult<Option<Session>> {
    utils_service::session_get(state.inner()).await
}
