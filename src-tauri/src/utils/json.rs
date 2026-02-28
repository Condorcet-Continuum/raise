// FICHIER : src-tauri/src/utils/json.rs

use crate::raise_error;
use crate::utils::RaiseResult;
use serde::de::DeserializeOwned;
use serde::Serialize;

// --- RE-EXPORTS (Single Source of Truth pour le JSON) ---
pub use serde_json::{json, Map, Value};

/// Parse une chaîne JSON en un type T.
/// Capture l'erreur de parsing avec un extrait du contenu en cas d'échec.
pub fn parse<T: DeserializeOwned>(s: &str) -> RaiseResult<T> {
    match serde_json::from_str(s) {
        Ok(val) => Ok(val),
        Err(e) => {
            // On capture un extrait du JSON pour aider au débogage
            let snippet = if s.len() > 100 { &s[..100] } else { s };
            raise_error!(
                "ERR_JSON_PARSE",
                error = e,
                context = json!({ "snippet": snippet })
            );
        }
    }
}

/// Convertit un type T en chaîne JSON compacte.
pub fn stringify<T: Serialize>(v: &T) -> RaiseResult<String> {
    match serde_json::to_string(v) {
        Ok(s) => Ok(s),
        Err(e) => raise_error!("ERR_JSON_STRINGIFY", error = e),
    }
}

/// Convertit un type T en chaîne JSON formatée (pretty).
pub fn stringify_pretty<T: Serialize>(v: &T) -> RaiseResult<String> {
    match serde_json::to_string_pretty(v) {
        Ok(s) => Ok(s),
        Err(e) => raise_error!("ERR_JSON_STRINGIFY_PRETTY", error = e),
    }
}

/// Convertit un type T en buffer binaire (Vec<u8>).
pub fn to_binary<T: Serialize>(v: &T) -> RaiseResult<Vec<u8>> {
    match serde_json::to_vec(v) {
        Ok(b) => Ok(b),
        Err(e) => raise_error!("ERR_JSON_TO_BINARY", error = e),
    }
}

/// Désérialise un type T depuis un buffer binaire.
pub fn from_binary<T: DeserializeOwned>(b: &[u8]) -> RaiseResult<T> {
    match serde_json::from_slice(b) {
        Ok(val) => Ok(val),
        Err(e) => raise_error!("ERR_JSON_FROM_BINARY", error = e),
    }
}

/// Convertit un `serde_json::Value` en type T.
pub fn from_value<T: DeserializeOwned>(v: Value) -> RaiseResult<T> {
    match serde_json::from_value(v) {
        Ok(val) => Ok(val),
        Err(e) => raise_error!("ERR_JSON_FROM_VALUE", error = e),
    }
}

/// Convertit un type T en `serde_json::Value`.
pub fn to_value<T: Serialize>(v: T) -> RaiseResult<Value> {
    match serde_json::to_value(v) {
        Ok(val) => Ok(val),
        Err(e) => raise_error!("ERR_JSON_TO_VALUE", error = e),
    }
}

/// Alias pour `to_binary` pour la compatibilité legacy.
pub fn to_vec<T: Serialize>(v: &T) -> RaiseResult<Vec<u8>> {
    to_binary(v)
}

/// Fusionne récursivement deux objets JSON (Deep Merge).
/// L'objet `b` écrase les valeurs de `a` en cas de conflit.
pub fn merge(a: &mut Value, b: Value) {
    match (a, b) {
        (Value::Object(a), Value::Object(b)) => {
            for (k, v) in b {
                merge(a.entry(k).or_insert(Value::Null), v);
            }
        }
        (a, b) => *a = b,
    }
}

// --- CONTEXT BUILDER ---
/*
/// Utilitaire fluide pour construire des contextes JSON pour les erreurs et les logs.
#[derive(Default)]
pub struct ContextBuilder {
    map: Map<String, Value>,
}

impl ContextBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add<S: Into<String>, V: Serialize>(mut self, key: S, value: V) -> Self {
        if let Ok(val) = serde_json::to_value(value) {
            self.map.insert(key.into(), val);
        }
        self
    }

    pub fn build(self) -> Value {
        Value::Object(self.map)
    }
}
*/
// --- TESTS UNITAIRES (RAISE standard) ---
#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::prelude::*;

    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    struct User {
        id: u32,
        role: String,
    }

    #[test]
    fn test_parse_success() {
        let raw = r#"{"id": 1, "role": "admin"}"#;
        let user: User = parse(raw).unwrap();
        assert_eq!(user.id, 1);
    }

    #[test]
    fn test_parse_error_structured() {
        let bad_raw = r#"{"id": "not_a_number"}"#;
        let res: RaiseResult<User> = parse(bad_raw);

        assert!(res.is_err());
        if let Err(AppError::Structured(data)) = res {
            assert_eq!(data.code, "ERR_JSON_PARSE");
            assert!(data.context.get("snippet").is_some());
        } else {
            panic!("Devrait retourner une erreur structurée");
        }
    }

    #[test]
    fn test_deep_merge() {
        let mut base = json!({ "api": { "port": 8080, "host": "localhost" }, "db": "prod" });
        let update = json!({ "api": { "port": 9000 }, "db": "staging" });

        merge(&mut base, update);

        assert_eq!(base["api"]["port"], 9000);
        assert_eq!(base["api"]["host"], "localhost");
        assert_eq!(base["db"], "staging");
    }
}
