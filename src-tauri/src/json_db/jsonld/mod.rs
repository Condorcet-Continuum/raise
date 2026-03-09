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
        type_: Option<String>,
        #[serde(rename = "@container")]
        container: Option<String>,
    },
}
