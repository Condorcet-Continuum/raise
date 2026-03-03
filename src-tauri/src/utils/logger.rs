// FICHIER : src-tauri/src/utils/logger.rs

use crate::utils::config::AppConfig;
use std::sync::Once;
use tracing_appender::rolling;
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Layer};

/// Sécurité pour éviter la double initialisation, critique lors de l'exécution
/// parallèle des tests unitaires.
static INIT: Once = Once::new();

/// Initialise le système de logging global de RAISE.
/// Configure une sortie console compacte et une sortie fichier JSON rotative.
pub fn init_logging() {
    INIT.call_once(|| {
        let config = AppConfig::get();

        // Résolution du chemin des logs
        let log_dir = config
            .get_path("PATH_RAISE_DOMAIN")
            .expect("ERREUR: PATH_RAISE_DOMAIN est introuvable dans la configuration !")
            .join("_system")
            .join("logs");

        // Création silencieuse du dossier de logs
        std::fs::create_dir_all(&log_dir).ok();

        // =========================================================================
        // LAYER 1 : FICHIER (Format JSON - Observabilité IA)
        // =========================================================================
        // Rotation journalière pour éviter l'explosion de la taille des fichiers
        let file_appender = rolling::daily(&log_dir, "raise.log");

        let file_layer = fmt::layer()
            .json()
            .with_writer(file_appender)
            .with_target(true)
            .with_thread_ids(true)
            .with_file(true)
            .with_line_number(true);

        // =========================================================================
        // LAYER 2 : CONSOLE (Format Compact - Pour l'Humain)
        // =========================================================================
        let env_filter = EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| EnvFilter::new("warn,user_notification=info"));

        let console_layer = fmt::layer()
            .compact()
            .with_target(false)
            .with_filter(env_filter);

        // =========================================================================
        // ASSEMBLAGE ET INITIALISATION DU REGISTRY
        // =========================================================================
        let registry = tracing_subscriber::registry()
            .with(file_layer)
            .with(console_layer);

        // On utilise try_init pour éviter de paniquer si un test a déjà initialisé le logger
        if let Err(_e) = registry.try_init() {
            return;
        }

        tracing::info!(
            "🚀 Logger RAISE V1.3 initialisé. Destination : {:?}",
            log_dir
        );
    });
}

// =========================================================================
// TESTS UNITAIRES (RAISE Standard)
// =========================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;
    use std::sync::{Arc, Mutex};
    use tracing_subscriber::fmt::MakeWriter;

    /// Mock d'écriture pour capturer et inspecter les logs en mémoire
    #[derive(Clone)]
    struct MemoryWriter {
        data: Arc<Mutex<Vec<u8>>>,
    }

    impl std::io::Write for MemoryWriter {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.data.lock().unwrap().extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    impl<'a> MakeWriter<'a> for MemoryWriter {
        type Writer = MemoryWriter;
        fn make_writer(&'a self) -> Self::Writer {
            self.clone()
        }
    }

    #[test]
    fn test_logger_init_idempotency() {
        // Vérifie que l'appel multiple ne provoque pas de panic
        let _ = init_logging();
        let _ = init_logging();
    }

    #[test]
    fn test_log_output_structure_validation() {
        // 1. Initialisation d'un subscriber de test isolé
        let log_data = Arc::new(Mutex::new(Vec::new()));
        let memory_writer = MemoryWriter {
            data: log_data.clone(),
        };

        let subscriber = tracing_subscriber::fmt()
            .json()
            .with_writer(memory_writer)
            .finish();

        // 2. Génération d'un log structuré (simulant une erreur RAISE)
        tracing::subscriber::with_default(subscriber, || {
            tracing::error!(
                service = "raise",
                component = "LOGGER_TEST",
                code = "ERR_TEST_STRUCTURE",
                action = "VALIDATE_JSON",
                "✅ Message de validation"
            );
        });

        // 3. Extraction et parsing
        let log_bytes = log_data.lock().unwrap();
        let log_str = String::from_utf8_lossy(&log_bytes);
        let first_line = log_str.lines().next().expect("Aucun log intercepté");

        let log: Value =
            serde_json::from_str(first_line).expect("Le log produit n'est pas un JSON valide");

        // 4. Validations des champs requis par le schéma Structured AI
        assert!(log.get("timestamp").is_some(), "Champ 'timestamp' absent");
        assert_eq!(log.get("level").and_then(|v| v.as_str()), Some("ERROR"));

        let fields = log.get("fields").expect("Champ 'fields' absent");

        assert_eq!(
            fields.get("service").and_then(|v| v.as_str()),
            Some("raise")
        );
        assert_eq!(
            fields.get("component").and_then(|v| v.as_str()),
            Some("LOGGER_TEST")
        );
        assert_eq!(
            fields.get("code").and_then(|v| v.as_str()),
            Some("ERR_TEST_STRUCTURE")
        );
        assert_eq!(
            fields.get("action").and_then(|v| v.as_str()),
            Some("VALIDATE_JSON")
        );

        assert!(
            fields.get("message").is_some(),
            "Le message final tracing est absent"
        );
    }
}
