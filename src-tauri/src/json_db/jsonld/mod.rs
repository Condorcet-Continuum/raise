// FICHIER : src-tauri/src/json_db/jsonld/mod.rs

//! Gestion des contextes JSON-LD pour données liées
//!
//! Ce module fournit des fonctions pour :
//! - Expansion : convertir JSON-LD compact en forme étendue
//! - Compaction : convertir forme étendue en JSON-LD compact
//! - Normalisation : produire des graphes RDF canoniques
//! - Validation : vérifier la conformité avec les schémas
use crate::utils::prelude::*;

pub mod context;
pub mod processor;
pub mod vocabulary;

// Re-exports pour l'usage externe
pub use self::context::{ArcadiaContext, ArcadiaLayer, ContextManager};
pub use self::processor::JsonLdProcessor;
pub use self::vocabulary::VocabularyRegistry;

/// Définition d'un contexte JSON-LD (pour sérialisation/désérialisation)
#[derive(Debug, Clone, Serializable, Deserializable)]
pub struct JsonLdContext {
    #[serde(rename = "@context")]
    pub context: ContextDefinition,
}

#[derive(Debug, Clone, Serializable, Deserializable)]
#[serde(untagged)]
pub enum ContextDefinition {
    Simple(String),
    Object(UnorderedMap<String, ContextJsonValue>),
    Array(Vec<ContextDefinition>),
}

#[derive(Debug, Clone, Serializable, Deserializable)]
#[serde(untagged)]
pub enum ContextJsonValue {
    Simple(String),
    Expanded {
        #[serde(rename = "@id")]
        id: Option<String>,
        #[serde(rename = "@type")]
        type_: Option<String>, // Supporte aussi les alias de types
        #[serde(rename = "@container")]
        container: Option<String>,
        #[serde(rename = "@language")]
        language: Option<String>,
    },
}

impl ContextJsonValue {
    pub fn get_iri(&self) -> Option<&str> {
        match self {
            Self::Simple(s) => Some(s),
            Self::Expanded { id, .. } => id.as_deref(),
        }
    }

    pub fn get_type(&self) -> Option<&str> {
        match self {
            Self::Expanded { type_, .. } => type_.as_deref(),
            _ => None,
        }
    }
}

// ============================================================================
// TESTS UNITAIRES
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    // 🎯 Utilisation de la façade Raise pour une gestion d'erreur unifiée
    use crate::utils::data::json::{deserialize_from_value, json_value, serialize_to_value};

    #[test]
    fn test_context_json_value_facade_integrity() -> RaiseResult<()> {
        // 1. Test Définition Simple (Alias String)
        let simple_json = json_value!("https://raise.io/oa#Activity");
        let val_simple: ContextJsonValue = deserialize_from_value(simple_json)?;

        assert_eq!(val_simple.get_iri(), Some("https://raise.io/oa#Activity"));
        assert!(val_simple.get_type().is_none());

        // 2. Test Définition Étendue (Objet JSON-LD)
        let expanded_json = json_value!({
            "@id": "https://raise.io/oa#Actor",
            "@type": "@id",
            "@container": "@set"
        });
        let val_ext: ContextJsonValue = deserialize_from_value(expanded_json)?;

        assert_eq!(val_ext.get_iri(), Some("https://raise.io/oa#Actor"));
        assert_eq!(val_ext.get_type(), Some("@id"));

        // 3. Test Roundtrip (Sérialisation -> Désérialisation)
        let original = ContextJsonValue::Expanded {
            id: Some("https://raise.io/oa#name".into()),
            type_: Some("xsd:string".into()),
            container: None,
            language: None, // ✅ FIX : On initialise explicitement le nouveau champ
        };

        // Conversion en JsonValue via la façade Raise
        let serialized = serialize_to_value(original.clone())?;
        assert_eq!(serialized["@id"], "https://raise.io/oa#name");

        // Retour à la structure originale
        let back: ContextJsonValue = deserialize_from_value(serialized)?;
        assert_eq!(back.get_iri(), original.get_iri());

        Ok(())
    }
}
