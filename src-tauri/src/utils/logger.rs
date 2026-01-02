use std::env;
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

/// Initialise le système de logging global.
/// À appeler une seule fois au début du `main.rs`.
pub fn init_logging() {
    // Si RUST_LOG n'est pas défini, on met un niveau par défaut raisonnable
    // On filtre pour voir les logs de "raise" en debug, et le reste en info
    if env::var("RUST_LOG").is_err() {
        env::set_var("RUST_LOG", "info,raise=debug");
    }

    // Configuration du formatteur (affichage compact pour le terminal)
    let fmt_layer = fmt::layer()
        .with_target(true) // Affiche le module source
        .with_thread_ids(false)
        .with_level(true)
        .with_file(false)
        .with_line_number(false)
        .compact();

    // Configuration du filtre (basé sur la variable d'env RUST_LOG)
    let filter_layer = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new("info"))
        .unwrap();

    // Initialisation du subscriber global
    tracing_subscriber::registry()
        .with(filter_layer)
        .with(fmt_layer)
        .try_init()
        .ok(); // On ignore l'erreur si déjà initialisé (utile pour les tests)

    tracing::info!("🚀 Système de logging initialisé.");
}

// ... code existant ...

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_logger_init_does_not_panic() {
        // On appelle init_logging.
        // Comme pour la config, tracing s'initialise une seule fois globalement.
        // On l'enveloppe pour ne pas faire échouer le test si c'est déjà fait.

        // Astuce : tracing::subscriber::set_global_default renvoie une erreur si déjà set.
        // Notre fonction init_logging() utilise .try_init().ok(), donc elle est "safe" à appeler plusieurs fois.

        init_logging();

        // Si on arrive ici sans crash, c'est gagné.
        tracing::info!(
            "Test du logger : ce message devrait apparaître lors de 'cargo test -- --nocapture'"
        );
    }
}
