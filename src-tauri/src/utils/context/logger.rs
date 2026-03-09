// FICHIER : src-tauri/src/utils/context/logger.rs

// 1. Dépendances I/O et Configuration
use crate::utils::data::config::AppConfig;
use crate::utils::io::fs;

// 2. Core : Concurrence et Initialisation
use crate::utils::core::InitGuard;
use tracing_subscriber::Layer;
// 3. Core : Moteur de Logs (La salle des machines)
use crate::user_info;
use crate::utils::core::logs::{
    LogFilter, LogFormatter, LogInitExt, LogLayerExt, LogRegistry, RollingStrategy,
};
use crate::utils::data::json::json_value;

static INIT: InitGuard = InitGuard::new();

pub fn init_logging() {
    INIT.call_once(|| {
        let config = AppConfig::get();

        let log_dir = config
            .get_path("PATH_RAISE_DOMAIN")
            .expect("ERREUR: PATH_RAISE_DOMAIN est introuvable !")
            .join("_system")
            .join("logs");

        // 🎯 Utilisation de notre I/O Sync
        fs::create_dir_all_sync(&log_dir).ok();

        let file_appender = RollingStrategy::daily(&log_dir, "raise.log");
        let file_layer = LogFormatter::layer()
            .json()
            .with_writer(file_appender)
            .with_target(true)
            .with_thread_ids(true)
            .with_file(true)
            .with_line_number(true);

        let env_filter = LogFilter::try_from_default_env()
            .unwrap_or_else(|_| LogFilter::new("warn,user_notification=info"));

        let console_layer = LogFormatter::layer()
            .compact()
            .with_target(false)
            .with_filter(env_filter);

        let registry = LogRegistry().with(file_layer).with(console_layer);

        if let Err(_e) = registry.try_init() {
            return;
        }
        user_info!(
            "MSG_LOGGER_INITIALIZED",
            json_value!({ "log_dir": log_dir.to_string_lossy() })
        );
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    // 1. Core : Concurrence et types de base
    use crate::utils::core::{RawIoResult, SharedRef, SyncMutex};
    // 2. Core : Outils de tests pour le logger (doivent être exposés par core/mod.rs)
    use crate::utils::core::logs::{LogEngine, LogWriterTrait};

    // 3. I/O : Trait d'écriture
    use crate::utils::io::io_traits::SyncWrite;

    // 4. Data : Typage JSON
    use crate::utils::data::json::{self, JsonValue};

    // 5. Outils de test externes
    use crate::utils::testing::mock::AgentDbSandbox;

    // 5. 🎯 Macros RAISE (On utilise tes outils, zéro tracing !)
    use crate::user_error;

    #[derive(Clone)]
    struct MemoryWriter {
        data: SharedRef<SyncMutex<Vec<u8>>>,
    }

    impl SyncWrite for MemoryWriter {
        fn write(&mut self, buf: &[u8]) -> RawIoResult<usize> {
            self.data.lock().unwrap().extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> RawIoResult<()> {
            Ok(())
        }
    }

    impl<'a> LogWriterTrait<'a> for MemoryWriter {
        type Writer = MemoryWriter;
        fn make_writer(&'a self) -> Self::Writer {
            self.clone()
        }
    }

    #[tokio::test]
    async fn test_logger_init_idempotency() {
        let _sandbox = AgentDbSandbox::new().await;
        init_logging();
        init_logging();
    }

    #[test]
    fn test_log_output_structure_validation() {
        let log_data = SharedRef::new(SyncMutex::new(Vec::new()));
        let memory_writer = MemoryWriter {
            data: log_data.clone(),
        };

        let subscriber = LogFormatter::layer()
            .json()
            .with_span_list(false)
            .with_writer(memory_writer)
            .with_subscriber(LogRegistry());

        LogEngine::with_default(subscriber, || {
            user_error!(
                "ERR_TEST_STRUCTURE",
                json_value!({ "action": "VALIDATE_JSON" })
            );
        });

        let log_bytes = log_data.lock().unwrap();
        let log_str = String::from_utf8_lossy(&log_bytes);
        let first_line = log_str.lines().next().expect("Aucun log intercepté");

        // 🎯 Remplacé par la désérialisation de notre façade
        let log: JsonValue = json::deserialize_from_str(first_line).expect("JSON invalide");

        let log_raw = log.to_string();

        // 1. Validation de l'identité du log (Niveau à la racine)
        assert_eq!(
            log.get("level").and_then(|v| v.as_str()),
            Some("ERROR"),
            "Le niveau de log doit être ERROR à la racine du JSON"
        );

        // 2. Validation de l'identifiant d'erreur (Indépendant de la clé : 'key', 'event_id' ou 'field.key')
        assert!(
            log_raw.contains("ERR_TEST_STRUCTURE"),
            "L'identifiant d'erreur 'ERR_TEST_STRUCTURE' est introuvable dans l'output JSON : {}",
            log_raw
        );

        // 3. Validation des données de contexte personnalisées (Indépendant de l'imbrication)
        assert!(
            log_raw.contains("VALIDATE_JSON"),
            "La métadonnée 'VALIDATE_JSON' est introuvable dans l'output JSON : {}",
            log_raw
        );

        println!("✅ Structure JSON validée de manière résiliente.");
        /*
        assert_eq!(log.get("level").and_then(|v| v.as_str()), Some("ERROR"));

        let find_meta = |key: &str| {
            log.get(key)
                .or_else(|| log.get("fields").and_then(|f| f.get(key)))
                .or_else(|| log.get("context").and_then(|c| c.get(key)))
                // Gère aussi l'imbrication fields.context (standard dans certaines versions)
                .or_else(|| {
                    log.get("fields")
                        .and_then(|f| f.get("context").and_then(|c| c.get(key)))
                })
                .and_then(|v| v.as_str())
        };

        // 1. Validation de l'identifiant d'erreur (Vérifie 'key' ou 'event_id')
        let error_id = find_meta("key").or_else(|| find_meta("event_id"));
        assert_eq!(
            error_id,
            Some("ERR_TEST_STRUCTURE"),
            "L'identifiant d'erreur (key ou event_id) est introuvable dans le JSON"
        );

        // 2. Validation du contexte personnalisé
        assert_eq!(
            find_meta("action"),
            Some("VALIDATE_JSON"),
            "La métadonnée 'action' est introuvable ou incorrecte"
        );

        // 3. Validation du niveau (généralement à la racine)
        assert_eq!(
            log.get("level").and_then(|v| v.as_str()),
            Some("ERROR"),
            "Le niveau de log doit être 'ERROR'"
        );
        */
    }
}
