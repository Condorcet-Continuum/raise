// FICHIER : src-tauri/src/utils/context/i18n.rs

// 1. Dépendances Métier (DB)
use crate::json_db::collections;
use crate::json_db::storage::{JsonDbConfig, StorageEngine};

// 2. Core : Concurrence et Erreurs
use crate::utils::core::error::RaiseResult;
use crate::utils::core::{SharedRef, StaticCell, SyncRwLock};
// 3. Données : Collections Sémantiques et Configuration
use crate::utils::data::config::AppConfig;
use crate::utils::data::json::{self, json_value};
use crate::utils::data::{Deserializable, UnorderedMap};

// 4. Macros RAISE Globales
use crate::raise_error;

// --- STRUCTURES DE DÉSÉRIALISATION (Internes) ---

#[derive(Debug, Deserializable)]
struct LocaleDocument {
    #[allow(dead_code)]
    locale: String,
    translations: Vec<TranslationItem>,
}

#[derive(Debug, Deserializable)]
struct TranslationItem {
    key: String,
    value: String,
}

// --- SINGLETON GLOBAL ---
// Utilisation de SharedRef<RwLock> pour garantir la thread-safety entre Tauri et les services
static TRANSLATOR: StaticCell<SharedRef<SyncRwLock<Translator>>> = StaticCell::new();

pub struct Translator {
    pub translations: UnorderedMap<String, String>, // 🎯 Remplacé
    pub current_lang: String,
}

impl Translator {
    fn new() -> Self {
        Self {
            translations: UnorderedMap::new(),
            current_lang: "en".to_string(),
        }
    }

    /// Charge une langue spécifique depuis la collection 'locales' en base de données.
    pub async fn load_from_db(&mut self, storage: &StorageEngine, lang: &str) -> RaiseResult<()> {
        let docs = match collections::list_all(storage, "_system", "_system", "locales").await {
            Ok(d) => d,
            Err(e) => raise_error!(
                "ERR_I18N_DB_READ",
                error = e,
                context = json_value!({ "requested_lang": lang }) // 🎯 Remplacé
            ),
        };

        for doc_val in docs {
            if doc_val.get("locale").and_then(|v| v.as_str()) == Some(lang) {
                // 🎯 Utilisation de notre façade json
                let document: LocaleDocument = match json::deserialize_from_value(doc_val) {
                    Ok(doc) => doc,
                    Err(e) => raise_error!(
                        "ERR_I18N_PARSE",
                        error = e,
                        context = json_value!({ "lang": lang })
                    ),
                };

                self.translations = document
                    .translations
                    .into_iter()
                    .map(|item| (item.key, item.value))
                    .collect();

                self.current_lang = lang.to_string();
                tracing::info!(
                    target: "system_core",
                    event_id = "I18N_LOCALE_LOADED",
                    language = lang,
                    key_count = self.translations.len(),
                    "🌍 Langue chargée depuis la base de données."
                );

                return Ok(());
            }
        }

        raise_error!(
            "ERR_I18N_NOT_FOUND",
            error = "Langue introuvable dans la collection 'locales'",
            context = json_value!({ "lang": lang })
        );
    }

    pub fn t(&self, key: &str) -> String {
        self.translations
            .get(key)
            .cloned()
            .unwrap_or_else(|| key.to_string())
    }
}

// --- INTERFACE PUBLIQUE ---

pub async fn init_i18n(lang: &str) -> RaiseResult<()> {
    let config = AppConfig::get();

    let Some(db_root) = config.get_path("PATH_RAISE_DOMAIN") else {
        raise_error!(
            "ERR_I18N_CONFIG_MISSING",
            error = "PATH_RAISE_DOMAIN est manquant dans la configuration",
            context = json_value!({ "lang": lang })
        );
    };

    let db_config = JsonDbConfig::new(db_root);
    let storage = StorageEngine::new(db_config);

    let mut temp_translator = Translator::new();
    temp_translator.load_from_db(&storage, lang).await?;

    let translator_handle =
        TRANSLATOR.get_or_init(|| SharedRef::new(SyncRwLock::new(Translator::new())));

    match translator_handle.write() {
        Ok(mut guard) => {
            guard.translations = temp_translator.translations;
            guard.current_lang = temp_translator.current_lang;
            Ok(())
        }
        Err(_) => raise_error!(
            "ERR_I18N_LOCK_POISONED",
            error = "Le verrou du traducteur est corrompu (poisoned)"
        ),
    }
}

pub fn t(key: &str) -> String {
    if let Some(arc) = TRANSLATOR.get() {
        if let Ok(read_guard) = arc.read() {
            return read_guard.t(key);
        }
    }
    key.to_string()
}

// --- TESTS UNITAIRES ---
#[cfg(test)]
mod tests {
    use super::*;
    use crate::json_db::collections::manager::CollectionsManager;
    use crate::utils::core::error::AppError;
    use crate::utils::core::UniqueId;
    use crate::utils::testing::mock::AgentDbSandbox;

    #[tokio::test]
    async fn test_translator_full_flow() {
        let sandbox = AgentDbSandbox::new().await;
        let manager = CollectionsManager::new(
            &sandbox.db,
            &sandbox.config.system_domain,
            &sandbox.config.system_db,
        );

        manager
            .create_collection(
                "locales",
                "db://_system/_system/schemas/v1/db/generic.schema.json",
            )
            .await
            .unwrap();

        let doc = json_value!({
            "_id": UniqueId::new_v4().to_string(),
            "locale": "fr",
            "translations": [
                { "key": "WELCOME", "value": "Bienvenue sur RAISE" },
                { "key": "ERROR", "value": "Une erreur est survenue" }
            ]
        });
        manager.insert_raw("locales", &doc).await.unwrap();

        let mut translator = Translator::new();
        translator
            .load_from_db(&sandbox.db, "fr")
            .await
            .expect("Echec load FR");

        assert_eq!(translator.current_lang, "fr");
        assert_eq!(translator.t("WELCOME"), "Bienvenue sur RAISE");
        assert_eq!(translator.t("UNKNOWN"), "UNKNOWN");
    }

    #[tokio::test]
    async fn test_translator_missing_language_error() {
        let sandbox = AgentDbSandbox::new().await;
        let manager = CollectionsManager::new(
            &sandbox.db,
            &sandbox.config.system_domain,
            &sandbox.config.system_db,
        );

        manager
            .create_collection(
                "locales",
                "db://_system/_system/schemas/v1/db/generic.schema.json",
            )
            .await
            .unwrap();

        let mut translator = Translator::new();
        let result = translator.load_from_db(&sandbox.db, "jp").await;

        assert!(result.is_err());
        let AppError::Structured(data) = result.unwrap_err();
        assert_eq!(data.code, "ERR_I18N_NOT_FOUND");
    }
}
