// FICHIER : src-tauri/src/utils/i18n.rs

use crate::json_db::collections;
// ✅ AJOUT : Import du StorageEngine en plus du JsonDbConfig
use crate::json_db::storage::{JsonDbConfig, StorageEngine};
use crate::raise_error; // Fondamental pour l'observabilité
use crate::utils::config::AppConfig;
use crate::utils::RaiseResult;
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::{Arc, OnceLock, RwLock};

// --- STRUCTURES DE DÉSÉRIALISATION (Internes) ---

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
// Utilisation d'Arc<RwLock> pour garantir la thread-safety entre Tauri et les services
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

    /// Charge une langue spécifique depuis la collection 'locales' en base de données.
    // ✅ MODIFICATION : Remplacement de db_config par storage
    pub async fn load_from_db(&mut self, storage: &StorageEngine, lang: &str) -> RaiseResult<()> {
        // 1. Récupération de tous les documents de la collection locales via le StorageEngine
        let docs = match collections::list_all(storage, "_system", "_system", "locales").await {
            Ok(d) => d,
            Err(e) => raise_error!(
                "ERR_I18N_DB_READ",
                error = e,
                context = serde_json::json!({ "requested_lang": lang })
            ),
        };

        // 2. Recherche du document correspondant à la langue cible
        for doc_val in docs {
            if doc_val.get("locale").and_then(|v| v.as_str()) == Some(lang) {
                // Interception des erreurs de structure (Schéma invalide en DB)
                let document: LocaleDocument = match serde_json::from_value(doc_val) {
                    Ok(doc) => doc,
                    Err(e) => raise_error!(
                        "ERR_I18N_PARSE",
                        error = e,
                        context = serde_json::json!({ "lang": lang })
                    ),
                };

                // Conversion de la liste en Map pour un accès O(1)
                self.translations = document
                    .translations
                    .into_iter()
                    .map(|item| (item.key, item.value))
                    .collect();

                self.current_lang = lang.to_string();

                tracing::info!(
                    "🌍 Langue chargée avec succès : {} ({} clés)",
                    lang,
                    self.translations.len()
                );

                return Ok(());
            }
        }

        // 3. Fallback si la langue n'existe pas
        raise_error!(
            "ERR_I18N_NOT_FOUND",
            error = "Langue introuvable dans la collection 'locales'",
            context = serde_json::json!({ "lang": lang })
        );
    }

    /// Traduit une clé. Retourne la clé brute si non trouvée.
    pub fn t(&self, key: &str) -> String {
        self.translations
            .get(key)
            .cloned()
            .unwrap_or_else(|| key.to_string())
    }
}

// --- INTERFACE PUBLIQUE ---

/// Initialise le système i18n global.
/// Doit être appelé après AppConfig::init().
pub async fn init_i18n(lang: &str) -> RaiseResult<()> {
    let config = AppConfig::get();

    // Extraction sécurisée du chemin de base
    let Some(db_root) = config.get_path("PATH_RAISE_DOMAIN") else {
        raise_error!(
            "ERR_I18N_CONFIG_MISSING",
            error = "PATH_RAISE_DOMAIN est manquant dans la configuration",
            context = serde_json::json!({ "lang": lang })
        );
    };

    let db_config = JsonDbConfig::new(db_root);
    // ✅ AJOUT : Instanciation du StorageEngine
    let storage = StorageEngine::new(db_config);

    let mut temp_translator = Translator::new();

    // ✅ MODIFICATION : Passage du StorageEngine
    temp_translator.load_from_db(&storage, lang).await?;

    // Mise à jour atomique du singleton
    let translator_handle = TRANSLATOR.get_or_init(|| Arc::new(RwLock::new(Translator::new())));

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

/// Traduit une clé via le traducteur global (Thread-safe).
pub fn t(key: &str) -> String {
    if let Some(arc) = TRANSLATOR.get() {
        if let Ok(read_guard) = arc.read() {
            return read_guard.t(key);
        }
    }
    // Fallback ultime : on retourne la clé
    key.to_string()
}

// --- TESTS UNITAIRES (RAISE standard) ---

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json_db::collections::manager::CollectionsManager;
    // 🎯 IMPORT UNIQUE : La Sandbox remplace tout le setup manuel !
    use crate::utils::mock::AgentDbSandbox;
    use crate::utils::prelude::*;
    use serde_json::json;

    #[tokio::test]
    async fn test_translator_full_flow() {
        // 1. 🎯 MAGIE : La Sandbox crée le dossier, initialise la DB et injecte les schémas
        let sandbox = AgentDbSandbox::new().await;

        let manager = CollectionsManager::new(
            &sandbox.db,
            &sandbox.config.system_domain,
            &sandbox.config.system_db,
        );

        // On a juste besoin de créer la collection "locales" pour ce test
        manager
            .create_collection(
                "locales",
                "db://_system/_system/schemas/v1/db/generic.schema.json",
            )
            .await
            .unwrap();

        // Insertion d'un document de langue valide
        let doc = json!({
            "id": Uuid::new_v4().to_string(),
            "locale": "fr",
            "translations": [
                { "key": "WELCOME", "value": "Bienvenue sur RAISE" },
                { "key": "ERROR", "value": "Une erreur est survenue" }
            ]
        });
        manager.insert_raw("locales", &doc).await.unwrap();

        let mut translator = Translator::new();

        // 2. 🎯 Utilisation directe de `&sandbox.db` (qui est un Arc<StorageEngine>)
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

        // On teste le cas d'erreur avec une langue inexistante
        let result = translator.load_from_db(&sandbox.db, "jp").await;

        assert!(result.is_err());
        if let Err(AppError::Structured(data)) = result {
            assert_eq!(data.code, "ERR_I18N_NOT_FOUND");
        } else {
            panic!("Devrait retourner ERR_I18N_NOT_FOUND structuré");
        }
    }
}
