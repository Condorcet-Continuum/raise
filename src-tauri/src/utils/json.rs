use anyhow::anyhow;
pub use serde::de::DeserializeOwned;

// --- RE-EXPORTS PUBLICS ---
// On re-exporte Serialize et Deserialize pour les rendre dispos via raise::utils::json
// Cela remplace les "use" simples qui étaient en conflit
pub use serde::{Deserialize, Serialize};

// On rend les types JSON accessibles (Value, Map, json!)
pub use serde_json::{json, Map, Value};

// On utilise notre Result unifié
use crate::utils::{AppError, Result};

/// Désérialise une chaîne JSON en structure typée
pub fn parse<T: DeserializeOwned>(content: &str) -> Result<T> {
    serde_json::from_str(content).map_err(|e| AppError::System(anyhow!("JSON Parse Error: {}", e)))
}

/// Convertit une Value en structure typée
pub fn from_value<T: DeserializeOwned>(value: Value) -> Result<T> {
    serde_json::from_value(value)
        .map_err(|e| AppError::System(anyhow!("JSON Conversion Error: {}", e)))
}

/// Convertit n'importe quelle structure sérialisable en Value JSON
pub fn to_value<T: Serialize>(value: T) -> Result<Value> {
    serde_json::to_value(value).map_err(|e| AppError::System(anyhow!("JSON to_value Error: {}", e)))
}

/// Désérialise des octets JSON en structure typée
pub fn from_slice<T: DeserializeOwned>(v: &[u8]) -> Result<T> {
    serde_json::from_slice(v)
        .map_err(|e| AppError::System(anyhow!("JSON Slice Parse Error: {}", e)))
}

/// Sérialise en String (Compact)
pub fn stringify<T: Serialize>(value: &T) -> Result<String> {
    serde_json::to_string(value)
        .map_err(|e| AppError::System(anyhow!("JSON Serialize Error: {}", e)))
}
pub fn to_vec<T: Serialize>(value: &T) -> Result<Vec<u8>> {
    // ✅ Result<Vec<u8>> au lieu de Result<String>
    serde_json::to_vec(value).map_err(|e| AppError::System(anyhow!("JSON To Vec Error: {}", e)))
}

/// Sérialise en String (Pretty Print - Standard pour RAISE)
pub fn stringify_pretty<T: Serialize>(value: &T) -> Result<String> {
    serde_json::to_string_pretty(value)
        .map_err(|e| AppError::System(anyhow!("JSON Serialize Error: {}", e)))
}

/// Sérialise un objet en binaire via Bincode (Format compact)
pub fn to_binary<T: Serialize>(data: &T) -> Result<Vec<u8>> {
    bincode::serde::encode_to_vec(data, bincode::config::standard())
        .map_err(|e| AppError::System(anyhow::anyhow!("Bincode Serialization Error: {}", e)))
}

/// Désérialise un objet binaire via Bincode
pub fn from_binary<T: DeserializeOwned>(bytes: &[u8]) -> Result<T> {
    bincode::serde::decode_from_slice(bytes, bincode::config::standard())
        .map_err(|e| AppError::System(anyhow::anyhow!("Bincode Deserialization Error: {}", e)))
        .map(|(data, _len)| data)
}
/// Effectue un "Deep Merge" de deux objets JSON.
/// 'a' est modifié sur place avec les valeurs de 'b'.
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

// --- CONTEXTE DE GÉNÉRATION (Data Fusion) ---

/// Constructeur fluide pour préparer les données destinées aux Templates (Tera) ou à l'IA.
/// Permet de fusionner Config, Modèle et Contexte sans douleur.
pub struct ContextBuilder {
    data: Value,
}

impl ContextBuilder {
    /// Crée un contexte vide.
    pub fn new() -> Self {
        Self { data: json!({}) }
    }

    /// Injecte la configuration globale (sous la clé "config").
    /// Utile pour avoir accès à {{ config.project_name }} dans les templates.
    pub fn with_config(mut self, config: &crate::utils::config::AppConfig) -> Self {
        // On convertit la config en Value pour pouvoir la fusionner
        if let Ok(cfg_val) = to_value(config) {
            merge(&mut self.data, json!({ "config": cfg_val }));
        }
        self
    }

    /// Injecte des données métier sous une clé spécifique.
    /// Ex: with_part("entity", &user_entity) -> {{ entity.name }}
    pub fn with_part(mut self, key: &str, value: &impl Serialize) -> Self {
        if let Ok(val) = to_value(value) {
            merge(&mut self.data, json!({ key: val }));
        }
        self
    }

    /// Fusionne un objet JSON arbitraire à la racine.
    /// Attention : peut écraser des clés existantes.
    pub fn merge_root(mut self, value: Value) -> Self {
        merge(&mut self.data, value);
        self
    }

    /// Finalise et retourne l'objet JSON complet.
    pub fn build(self) -> Value {
        self.data
    }
}

// Pour faciliter l'usage : ContextBuilder::default()
impl Default for ContextBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deep_merge() {
        let mut a = json!({
            "name": "Doc",
            "meta": { "version": 1, "active": true }
        });
        let b = json!({
            "meta": { "version": 2 } // Doit mettre à jour version mais garder active
        });

        merge(&mut a, b);

        assert_eq!(a["meta"]["version"], 2);
        assert_eq!(a["meta"]["active"], true);
        assert_eq!(a["name"], "Doc");
    }
    #[test]
    fn test_context_builder_fusion() {
        let mut builder = ContextBuilder::new();

        // 1. Données de base
        builder = builder.with_part("meta", &json!({ "version": "1.0", "author": "Zair" }));

        // 2. Fusion (Update)
        // On veut vérifier que "author" reste, mais "version" change
        builder = builder.merge_root(json!({
            "meta": { "version": "2.0" },
            "new_field": true
        }));

        let result = builder.build();

        assert_eq!(result["meta"]["version"], "2.0");
        assert_eq!(
            result["meta"]["author"], "Zair",
            "Les champs non modifiés doivent rester"
        );
        assert_eq!(result["new_field"], true);
    }
}
