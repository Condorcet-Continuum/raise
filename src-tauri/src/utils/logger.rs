use crate::utils::config::AppConfig;
use std::sync::Once;
use tracing_appender::rolling;
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Layer};

// Sécurité pour éviter la double initialisation (crash fréquent en tests)
static INIT: Once = Once::new();

pub fn init_logging() {
    INIT.call_once(|| {
        // 1. Configuration des chemins via AppConfig
        // Attention : AppConfig doit être initialisé avant d'appeler cette fonction !
        let config = AppConfig::get();

        // ADAPTATION : Changement du chemin vers <ROOT>/_system/_system/logs
        // On remplace config.database_root par config.paths.raise_domain
        let log_dir = config.get_path("PATH_RAISE_DOMAIN")
            .expect("ERREUR: PATH_RAISE_DOMAIN est introuvable dans la configuration !")
            .join("_system")
            .join("logs");

        // Création silencieuse du dossier logs s'il n'existe pas
        std::fs::create_dir_all(&log_dir).ok();

        // 2. Layer Fichier : Rotation journalière + Format JSON
        // Ce layer capture TOUT (Info, Warn, Error...) pour l'historique
        let file_appender = rolling::daily(&log_dir, "raise.log");

        let file_layer = fmt::layer()
            .json() // Format JSON structuré conservé
            .with_writer(file_appender)
            .with_target(true) // Affiche le module (ex: raise::utils::i18n)
            .with_thread_ids(true) // Utile pour le debug async
            .with_file(true) // Fichier source
            .with_line_number(true); // Ligne du code

        // 3. Layer Console : Nettoyé pour l'UX
        // Par défaut, on n'affiche que les WARNINGS et ERREURS techniques.
        let env_filter =
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("warn"));

        let console_layer = fmt::layer()
            .compact() // Format plus court conservé
            .with_target(false) // On cache le module technique à l'utilisateur
            .with_filter(env_filter);

        // 4. Assemblage et Initialisation (Sécurisée)
        let registry = tracing_subscriber::registry()
            .with(file_layer)
            .with(console_layer);

        // CORRECTION : On utilise try_init() pour ne pas paniquer si un autre test
        // a déjà initialisé le tracing globalement.
        if let Err(_e) = registry.try_init() {
            // Si on est ici, c'est que le logging est déjà actif.
            // On utilise tracing::warn! au lieu de eprintln! car un subscriber existe forcément.
            tracing::warn!("⚠️ [Logger] Tentative de ré-initialisation ignorée (Global subscriber déjà actif).");
            return;
        }

        // Ce log partira dans le fichier (INFO), mais ne s'affichera pas en console (WARN par défaut)
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
        // PRÉ-REQUIS : On doit initialiser AppConfig car le logger en a besoin.
        // Note: AppConfig::init() peut échouer si déjà init, on ignore l'erreur
        if AppConfig::init().is_err() {
            println!("⚠️ AppConfig n'a pas pu s'initialiser (environnement de test minimal). Test ignoré proprement.");
            return;
        }

        // Le test réel commence ici
        init_logging();
        init_logging(); // Le second appel ne doit plus paniquer grâce à try_init()
    }
}
