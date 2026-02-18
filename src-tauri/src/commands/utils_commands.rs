use crate::utils::prelude::*;

use tauri::command;

/// Structure de réponse renvoyée au Frontend
#[derive(Debug, Serialize)]
pub struct SystemInfoResponse {
    pub app_version: String,
    pub env_mode: String,
    pub api_status: String,
    pub database_path: String,
}

/// Commande Tauri : Récupère les informations système
/// Retourne un Result<SystemInfoResponse, AppError> qui sera sérialisé en JSON ou string d'erreur.
#[command]
pub async fn get_app_info() -> Result<SystemInfoResponse> {
    // 1. Log structuré (visible si RUST_LOG=info ou debug)
    tracing::info!("📥 Commande reçue : get_app_info");

    // 2. Accès sécurisé à la configuration V2
    let config = AppConfig::get();

    // 3. Récupération dynamique du moteur IA principal (remplace l'ancien config.llm)
    let primary_llm = config.ai_engines.get("primary_local").ok_or_else(|| {
        AppError::Config("Moteur 'primary_local' introuvable dans la configuration".to_string())
    })?;

    // On extrait l'URL de l'API en gérant le fait que c'est une Option<String> dans la V2
    let api_url = primary_llm.api_url.as_deref().unwrap_or("");

    if api_url.is_empty() {
        tracing::error!("URL de l'API LLM manquante !");
        return Err(AppError::Config(
            "URL API LLM non configurée pour primary_local".to_string(),
        ));
    }

    // 4. Récupération dynamique du chemin du domaine (remplace config.paths.raise_domain)
    let raise_domain_path = config
        .get_path("PATH_RAISE_DOMAIN")
        .expect("ERREUR: PATH_RAISE_DOMAIN est introuvable dans la configuration !")
        .to_string_lossy()
        .to_string();

    // 5. Construction de la réponse
    let response = SystemInfoResponse {
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        env_mode: config.core.env_mode.clone(), // On utilise le mode lu depuis le JSON !
        api_status: format!("Connecté à {}", api_url),
        database_path: raise_domain_path,
    };

    tracing::debug!("✅ Réponse envoyée : {:?}", response);
    Ok(response)
}
