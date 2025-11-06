//! Gestion des contextes JSON-LD pour données liées

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub mod context;
pub mod processor;
pub mod vocabulary;

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
        id: String,
        #[serde(rename = "@type")]
        type_: Option<String>,
        #[serde(rename = "@container")]
        container: Option<String>,
    },
}
