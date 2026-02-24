use crate::json_db::collections;
use crate::json_db::storage::JsonDbConfig;
use crate::utils::config::AppConfig;
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::{Arc, OnceLock, RwLock};

// --- STRUCTURES DE DÉSÉRIALISATION ---
#[derive(Debug, Deserialize)]
struct LocaleDocument {
    #[allow(dead_code)]
    locale: String,
    translations: Vec<TranslationItem>,
}

#[derive(Debug, Deserialize)]
struct TranslationItem {
    key: String,
    value: String,
}

// --- SINGLETON GLOBAL ---
static TRANSLATOR: OnceLock<Arc<RwLock<Translator>>> = OnceLock::new();

pub struct Translator {
    pub translations: HashMap<String, String>,
    pub current_lang: String,
}

impl Translator {
    fn new() -> Self {
        Self {
            translations: HashMap::new(),
            current_lang: "en".to_string(),
        }
    }

    /// Charge la langue depuis la base de données (Testable en isolation)
    pub async fn load_from_db(&mut self, db_config: &JsonDbConfig, lang: &str) {
        match collections::list_all(db_config, "_system", "_system", "locales").await {
            Ok(docs) => {
                for doc_val in docs {
                    if doc_val.get("locale").and_then(|v| v.as_str()) == Some(lang) {
                        if let Ok(document) = serde_json::from_value::<LocaleDocument>(doc_val) {
                            let map: HashMap<String, String> = document
                                .translations
                                .into_iter()
                                .map(|item| (item.key, item.value))
                                .collect();

                            self.translations = map;
                            self.current_lang = lang.to_string();
                            tracing::info!(
                                "🌍 Langue chargée avec succès : {} ({} clés)",
                                lang,
                                self.translations.len()
                            );
                            return; // On a trouvé et chargé la langue, on s'arrête
                        }
                    }
                }
                tracing::warn!(
                    "⚠️ Langue '{}' introuvable dans la collection 'locales'.",
                    lang
                );
            }
            Err(e) => tracing::error!("❌ Erreur JsonDb lors du chargement des locales : {}", e),
        }
    }

    pub fn t(&self, key: &str) -> String {
        self.translations
            .get(key)
            .cloned()
            .unwrap_or_else(|| key.to_string())
    }
}

/// Initialise le système global avec une langue cible (ASYNC)
pub async fn init_i18n(lang: &str) {
    let config = AppConfig::get();
    let db_root = config
        .get_path("PATH_RAISE_DOMAIN")
        .expect("ERREUR: PATH_RAISE_DOMAIN est introuvable dans la configuration !");

    let db_config = JsonDbConfig::new(db_root);

    // 1. On prépare le nouveau dictionnaire de manière asynchrone
    let mut temp_translator = Translator::new();
    temp_translator.load_from_db(&db_config, lang).await;

    // 2. MISE À JOUR SYNCHRONE ULTRA-RAPIDE DU GLOBAL
    let translator = TRANSLATOR.get_or_init(|| Arc::new(RwLock::new(Translator::new())));
    if let Ok(mut write_guard) = translator.write() {
        write_guard.translations = temp_translator.translations;
        write_guard.current_lang = temp_translator.current_lang;
    }
}

/// Helper public : Traduit une clé via l'instance globale (Sync)
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
    use crate::json_db::storage::{JsonDbConfig, StorageEngine};
    use serde_json::json;

    // Helper pour générer un environnement de DB temporaire
    fn setup_env() -> (tempfile::TempDir, JsonDbConfig) {
        crate::utils::config::test_mocks::inject_mock_config();
        let dir = tempfile::tempdir().unwrap();
        let config = JsonDbConfig::new(dir.path().to_path_buf());
        (dir, config)
    }

    #[test]
    fn test_translator_default_behavior() {
        // Par défaut, sans base de données, il renvoie la clé
        let translator = Translator::new();
        assert_eq!(translator.t("HELLO"), "HELLO");
        assert_eq!(translator.current_lang, "en");
    }

    #[tokio::test] // On utilise tokio car on teste des appels asynchrones
    async fn test_translator_db_integration() {
        let (_dir, db_config) = setup_env();
        let storage = StorageEngine::new(db_config.clone());
        let manager = CollectionsManager::new(&storage, "_system", "_system");

        // 1. Initialisation de la fausse base de données
        manager.init_db().await.unwrap();
        manager.create_collection("locales", None).await.unwrap();

        // 2. Création et insertion d'un faux document de traduction
        let doc = json!({
            "id": "uuid-test-fr",
            "locale": "fr",
            "translations": [
                { "key": "HELLO", "value": "Bonjour" },
                { "key": "BYE", "value": "Au revoir" }
            ]
        });
        manager.insert_raw("locales", &doc).await.unwrap();

        // 3. Test du chargement via le Translator
        let mut translator = Translator::new();
        translator.load_from_db(&db_config, "fr").await;

        // 4. Vérifications
        assert_eq!(translator.current_lang, "fr");
        assert_eq!(translator.t("HELLO"), "Bonjour");
        assert_eq!(translator.t("BYE"), "Au revoir");
        assert_eq!(translator.t("UNKNOWN_KEY"), "UNKNOWN_KEY");
    }

    #[test]
    fn test_global_access_fallback() {
        // Si non initialisé, le fallback renvoie la clé
        let result = t("TEST_KEY");
        assert_eq!(result, "TEST_KEY");
    }
}
