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
use crate::utils::data::{Deserializable, OrderedMap, Serializable, UnorderedMap};

// 4. Macros RAISE Globales
use crate::raise_error;

/// 🎯 TYPE SÉMANTIQUE RAISE : Gère les chaînes multilingues du Knowledge Graph.
/// Aligné sur 'i18nNonEmptyString' des schémas JSON.
#[derive(Debug, Clone, Serializable, Deserializable, PartialEq, Eq)]
#[serde(untagged)]
pub enum I18nString {
    /// Format simple : "Mon Nom"
    Single(String),
    /// Format multilingue : {"fr": "Mon Nom", "en": "My Name"}
    Multi(OrderedMap<String, String>),
}

impl I18nString {
    /// Récupère la traduction pour une langue donnée avec fallback sécurisé (sans panique).
    pub fn get_text(&self, lang: &str) -> String {
        match self {
            Self::Single(s) => s.clone(),
            Self::Multi(map) => map
                .get(lang)
                .or_else(|| map.get("en"))
                .or_else(|| map.values().next())
                .cloned()
                .unwrap_or_default(),
        }
    }
}

// Implémentation pratique
impl From<&str> for I18nString {
    fn from(s: &str) -> Self {
        Self::Single(s.to_string())
    }
}

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
static TRANSLATOR: StaticCell<SharedRef<SyncRwLock<Translator>>> = StaticCell::new();

pub struct Translator {
    pub translations: UnorderedMap<String, String>,
    pub current_lang: String,
}

impl Translator {
    fn new() -> Self {
        Self {
            translations: UnorderedMap::new(),
            current_lang: "en".to_string(),
        }
    }

    /// Charge une langue spécifique depuis la collection 'locales' du point de montage système.
    pub async fn load_from_db(&mut self, storage: &StorageEngine, lang: &str) -> RaiseResult<()> {
        let app_config = AppConfig::get();

        let (target_domain, target_db, target_collection) =
            app_config.resolve_system_uri(app_config.system_assets.locales_uri.as_ref(), "locales");

        // 🎯 Rigueur : Match complet sur la lecture DB
        let docs =
            match collections::list_all(storage, &target_domain, &target_db, &target_collection)
                .await
            {
                Ok(d) => d,
                Err(e) => raise_error!(
                    "ERR_I18N_DB_READ",
                    error = e,
                    context = json_value!({
                        "requested_lang": lang,
                        "resolved_domain": target_domain,
                        "resolved_db": target_db,
                        "resolved_collection": target_collection,
                        "uri_source": app_config.system_assets.locales_uri
                    })
                ),
            };

        for doc_val in docs {
            if doc_val.get("locale").and_then(|v| v.as_str()) == Some(lang) {
                // 🎯 Rigueur : Match complet sur la désérialisation
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
                    source = if app_config.system_assets.locales_uri.is_some() { "shared_registry" } else { "mount_point_fallback" },
                    "🌍 Langue chargée avec succès."
                );

                return Ok(());
            }
        }

        raise_error!(
            "ERR_I18N_NOT_FOUND",
            error = format!(
                "Langue '{}' introuvable dans la collection '{}'",
                lang, target_collection
            ),
            context = json_value!({ "lang": lang, "collection": target_collection })
        );
    }

    pub fn t(&self, key: &str) -> String {
        match self.translations.get(key) {
            Some(val) => val.clone(),
            None => key.to_string(), // Fallback sur la clé technique si absente
        }
    }
}

// --- INTERFACE PUBLIQUE ---

pub async fn init_i18n(lang: &str) -> RaiseResult<()> {
    let config = AppConfig::get();

    // 🎯 Rigueur : Match complet au lieu de let-else
    let db_root = match config.get_path("PATH_RAISE_DOMAIN") {
        Some(p) => p,
        None => raise_error!(
            "ERR_I18N_CONFIG_MISSING",
            error = "PATH_RAISE_DOMAIN est manquant dans la configuration",
            context = json_value!({ "lang": lang })
        ),
    };

    let db_config = JsonDbConfig::new(db_root);
    let storage = StorageEngine::new(db_config)?;

    let mut temp_translator = Translator::new();
    temp_translator.load_from_db(&storage, lang).await?;

    let translator_handle =
        TRANSLATOR.get_or_init(|| SharedRef::new(SyncRwLock::new(Translator::new())));

    // 🎯 Rigueur : Match complet sur le verrou
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
    match TRANSLATOR.get() {
        Some(arc) => match arc.read() {
            Ok(read_guard) => read_guard.t(key),
            Err(_) => key.to_string(), // Fallback minimal de sécurité
        },
        None => key.to_string(),
    }
}

// --- TESTS UNITAIRES ---
#[cfg(test)]
mod tests {
    use super::*;
    use crate::json_db::collections::manager::CollectionsManager;
    use crate::utils::core::error::AppError;
    use crate::utils::core::UniqueId;
    use crate::utils::testing::mock::AgentDbSandbox;

    // 🎯 Modification : Les tests renvoient un RaiseResult pour utiliser l'opérateur '?' au lieu de unwrap()
    #[tokio::test]
    async fn test_translator_full_flow() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await?;

        let manager = CollectionsManager::new(
            &sandbox.db,
            &sandbox.config.mount_points.system.domain,
            &sandbox.config.mount_points.system.db,
        );

        manager
            .create_collection("locales", "v1/db/generic.schema.json")
            .await?;

        let doc = json_value!({
            "_id": UniqueId::new_v4().to_string(),
            "locale": "fr",
            "translations": [
                { "key": "WELCOME", "value": "Bienvenue sur RAISE" },
                { "key": "ERROR", "value": "Une erreur est survenue" }
            ]
        });

        manager.insert_raw("locales", &doc).await?;

        let mut translator = Translator::new();
        match translator.load_from_db(&sandbox.db, "fr").await {
            Ok(_) => {}
            Err(e) => panic!("Échec inattendu du chargement FR: {:?}", e),
        };

        assert_eq!(translator.current_lang, "fr");
        assert_eq!(translator.t("WELCOME"), "Bienvenue sur RAISE");
        assert_eq!(translator.t("UNKNOWN"), "UNKNOWN");

        Ok(())
    }

    #[tokio::test]
    async fn test_translator_missing_language_error() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await?;

        // 🎯 FIX : Utilisation des nouveaux mount_points
        let manager = CollectionsManager::new(
            &sandbox.db,
            &sandbox.config.mount_points.system.domain,
            &sandbox.config.mount_points.system.db,
        );

        manager
            .create_collection("locales", "v1/db/generic.schema.json")
            .await?;

        let mut translator = Translator::new();

        // 🎯 Rigueur : Match pour intercepter précisément l'erreur attendue
        match translator.load_from_db(&sandbox.db, "jp").await {
            Ok(_) => panic!("Le chargement d'une langue inexistante aurait dû échouer."),
            Err(AppError::Structured(data)) => {
                assert_eq!(data.code, "ERR_I18N_NOT_FOUND");
            }
        }

        Ok(())
    }
}
