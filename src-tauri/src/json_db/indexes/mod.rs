//! Système d'indexation pour requêtes rapides

pub mod btree_index;
pub mod hash_index;
pub mod text_index;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Index {
    pub name: String,
    pub collection: String,
    pub fields: Vec<String>,
    pub index_type: IndexType,
    pub unique: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum IndexType {
    BTree,
    Hash,
    Text,
}
