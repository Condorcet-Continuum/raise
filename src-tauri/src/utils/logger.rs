// FICHIER : src-tauri/src/utils/logger.rs

use crate::utils::config::AppConfig;
use std::sync::Once;
use tracing_appender::rolling;
use tracing_subscriber::{
    filter::filter_fn, fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Layer,
};

// Sécurité pour éviter la double initialisation (crash fréquent en tests)
static INIT: Once = Once::new();

pub fn init_logging() {
    INIT.call_once(|| {
        let config = AppConfig::get();

        let log_dir = config
            .get_path("PATH_RAISE_DOMAIN")
            .expect("ERREUR: PATH_RAISE_DOMAIN est introuvable dans la configuration !")
            .join("_system")
            .join("logs");

        std::fs::create_dir_all(&log_dir).ok();

        // =========================================================================
        // LAYER 1 : FICHIER (Pour les Agents IA)
        // =========================================================================
        let file_appender = rolling::daily(&log_dir, "raise.log");

        let file_layer = fmt::layer()
            .json()
            .with_writer(file_appender)
            .with_target(true)
            .with_thread_ids(true)
            .with_file(true)
            .with_line_number(true);

        // =========================================================================
        // LAYER 2 : CONSOLE (Pour l'Humain)
        // =========================================================================
        let env_filter =
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("warn"));

        // Filtre anti-doublon pour ne pas polluer la console avec les logs des macros
        let anti_double_filter = filter_fn(|metadata| {
            !metadata.fields().iter().any(|f| f.name() == "event")
        });

        let console_layer = fmt::layer()
            .compact()
            .with_target(false)
            .with_filter(env_filter)
            .with_filter(anti_double_filter);

        // =========================================================================
        // ASSEMBLAGE ET INITIALISATION
        // =========================================================================
        let registry = tracing_subscriber::registry()
            .with(file_layer)
            .with(console_layer);

        if let Err(_e) = registry.try_init() {
            tracing::warn!("⚠️ [Logger] Tentative de ré-initialisation ignorée (Global subscriber déjà actif).");
            return;
        }

        tracing::info!(
            "🚀 Logger initialisé. Logs disponibles dans : {:?}",
            log_dir
        );
    });
}

// --- TESTS UNITAIRES ---
#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::config::AppConfig;

    #[test]
    fn test_logger_init_idempotency() {
        if AppConfig::init().is_err() {
            println!("⚠️ AppConfig n'a pas pu s'initialiser. Test ignoré proprement.");
            return;
        }

        init_logging();
        init_logging();
    }
}
