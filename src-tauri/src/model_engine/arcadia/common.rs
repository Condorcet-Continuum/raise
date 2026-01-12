use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Référence vers un autre élément (par son UUID)
pub type ElementRef = String;

/// Chaîne internationalisée (ou simple string selon le cas)
/// Permet de gérer "Nom" ou {"fr": "Nom", "en": "Name"}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum I18nString {
    String(String),
    Map(HashMap<String, String>),
}

impl Default for I18nString {
    fn default() -> Self {
        I18nString::String("".to_string())
    }
}

/// Socle technique commun à tous les éléments Arcadia
/// Correspond aux attributs techniques (ID, dates de modif...)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BaseEntity {
    pub id: String,

    #[serde(rename = "created", default)]
    pub created_at: String,

    #[serde(rename = "modified", default)]
    pub modified_at: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_i18n_string_serialization() {
        // Test String simple
        let simple = I18nString::String("Bonjour".to_string());
        assert_eq!(serde_json::to_value(&simple).unwrap(), json!("Bonjour"));

        // Test Map
        let mut map = HashMap::new();
        map.insert("en".to_string(), "Hello".to_string());
        let complex = I18nString::Map(map);
        assert_eq!(
            serde_json::to_value(&complex).unwrap(),
            json!({"en": "Hello"})
        );
    }

    #[test]
    fn test_base_entity() {
        let entity = BaseEntity {
            id: "uuid-123".to_string(),
            created_at: "2023-01-01".to_string(),
            modified_at: "2023-01-02".to_string(),
        };
        let json = serde_json::to_string(&entity).unwrap();
        assert!(json.contains("uuid-123"));
        assert!(json.contains("created")); // Vérifie le rename
    }
}
