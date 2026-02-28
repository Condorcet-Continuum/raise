// FICHIER : src-tauri/src/json_db/jsonld/mod.rs

//! Gestion des contextes JSON-LD pour données liées
//!
//! Ce module fournit des fonctions pour :
//! - Expansion : convertir JSON-LD compact en forme étendue
//! - Compaction : convertir forme étendue en JSON-LD compact
//! - Normalisation : produire des graphes RDF canoniques
//! - Validation : vérifier la conformité avec les schémas
use crate::utils::prelude::*;
use crate::utils::{
    // Serde via la façade
    HashMap, // Collection via la façade
};

pub mod context;
pub mod processor;
pub mod vocabulary;

// Re-exports pour l'usage externe
pub use self::context::{ArcadiaContext, ArcadiaLayer, ContextManager};
pub use self::processor::JsonLdProcessor;
pub use self::vocabulary::VocabularyRegistry;

/// Définition d'un contexte JSON-LD (pour sérialisation/désérialisation)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonLdContext {
    #[serde(rename = "@context")]
    pub context: ContextDefinition,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ContextDefinition {
    Simple(String),
    Object(HashMap<String, ContextValue>),
    Array(Vec<ContextDefinition>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ContextValue {
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
