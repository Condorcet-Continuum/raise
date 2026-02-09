use crate::utils::config::AppConfig;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, OnceLock, RwLock};

// Singleton global thread-safe : Une seule instance pour toute l'app
static TRANSLATOR: OnceLock<Arc<RwLock<Translator>>> = OnceLock::new();

/// Structure interne qui détient les données
pub struct Translator {
    translations: HashMap<String, String>,
    pub current_lang: String,
}

impl Translator {
    fn new() -> Self {
        Self {
            translations: HashMap::new(),
            current_lang: "en".to_string(), // Langue par défaut technique
        }
    }

    /// Charge un fichier de langue depuis le disque
    /// Le chemin est calculé via AppConfig + /locales/{lang}.json
    pub fn load(&mut self, lang: &str) {
        // On récupère la racine de la DB/Config
        let config = AppConfig::get();
        let locales_dir = config.database_root.join("locales");
        let path = locales_dir.join(format!("{}.json", lang));

        self.load_from_path(lang, path);
    }

    /// Méthode interne pour charger depuis un chemin spécifique (utile pour les tests)
    fn load_from_path(&mut self, lang: &str, path: PathBuf) {
        if path.exists() {
            match fs::read_to_string(&path) {
                Ok(content) => {
                    // On attend un simple dictionnaire clé/valeur : {"HELLO": "Bonjour"}
                    match serde_json::from_str::<HashMap<String, String>>(&content) {
                        Ok(map) => {
                            self.translations = map;
                            self.current_lang = lang.to_string();
                            tracing::info!("🌍 Langue chargée : {} (depuis {:?})", lang, path);
                        }
                        Err(e) => {
                            tracing::error!("❌ Erreur parsing JSON langue ({:?}): {}", path, e);
                        }
                    }
                }
                Err(e) => {
                    tracing::error!(
                        "❌ Impossible de lire le fichier langue ({:?}): {}",
                        path,
                        e
                    );
                }
            }
        } else {
            tracing::warn!("⚠️ Fichier de traduction introuvable : {:?}", path);
        }
    }

    pub fn t(&self, key: &str) -> String {
        self.translations
            .get(key)
            .cloned()
            .unwrap_or_else(|| key.to_string())
    }
}

/// Initialise le système global avec une langue cible
pub fn init_i18n(lang: &str) {
    let translator = TRANSLATOR.get_or_init(|| Arc::new(RwLock::new(Translator::new())));

    // Verrouillage en écriture pour mettre à jour la langue
    if let Ok(mut write_guard) = translator.write() {
        write_guard.load(lang);
    }
}

/// Helper public : Traduit une clé via l'instance globale
pub fn t(key: &str) -> String {
    if let Some(arc) = TRANSLATOR.get() {
        // Verrouillage en lecture (très rapide)
        if let Ok(read_guard) = arc.read() {
            return read_guard.t(key);
        }
    }
    // Fallback si le système n'est pas encore init
    key.to_string()
}

// --- TESTS UNITAIRES ---
#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_translator_default_behavior() {
        // Par défaut (sans chargement), le traducteur renvoie la clé
        let translator = Translator::new();
        assert_eq!(translator.t("HELLO"), "HELLO");
        assert_eq!(translator.current_lang, "en");
    }

    #[test]
    fn test_translator_loading_valid_json() {
        // 1. Création d'un fichier temporaire simulant fr.json
        let mut temp_file = NamedTempFile::new().unwrap();
        let json_content = r#"{
            "HELLO": "Bonjour",
            "BYE": "Au revoir"
        }"#;
        write!(temp_file, "{}", json_content).unwrap();

        // 2. Chargement manuel via load_from_path
        let mut translator = Translator::new();
        translator.load_from_path("fr", temp_file.path().to_path_buf());

        // 3. Vérifications
        assert_eq!(translator.current_lang, "fr");
        assert_eq!(translator.t("HELLO"), "Bonjour");
        assert_eq!(translator.t("BYE"), "Au revoir");
        assert_eq!(translator.t("UNKNOWN"), "UNKNOWN"); // Clé manquante = Clé
    }

    #[test]
    fn test_translator_loading_invalid_json() {
        let mut temp_file = NamedTempFile::new().unwrap();
        write!(temp_file, "INVALID JSON").unwrap();

        let mut translator = Translator::new();
        // Ne doit pas paniquer, juste logger une erreur
        translator.load_from_path("es", temp_file.path().to_path_buf());

        // Doit rester à l'état précédent ou vide
        assert_eq!(translator.t("HELLO"), "HELLO");
    }

    #[test]
    fn test_global_access() {
        // Test de la fonction statique t()
        let result = t("TEST_KEY");
        assert_eq!(result, "TEST_KEY");
    }
}
