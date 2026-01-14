// FICHIER : src-tauri/src/json_db/indexes/mod.rs

use serde::{Deserialize, Serialize};

// Modules d'implémentation
pub mod btree;
pub mod driver;
pub mod hash;
pub mod manager;
pub mod paths;
pub mod text;

pub use manager::IndexManager;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum IndexType {
    BTree,
    Hash,
    Text,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexDefinition {
    pub name: String,
    /// Pointeur JSON vers le champ (ex: "/email")
    pub field_path: String,
    pub index_type: IndexType,
    pub unique: bool,
}

/// Structure de stockage sur disque d'une entrée d'index
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IndexRecord {
    // On stocke la clé sous forme de String brute pour éviter les soucis de polymorphisme Bincode
    pub key: String,
    pub document_id: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_index_def_serialization() {
        let def = IndexDefinition {
            name: "email".into(),
            field_path: "/contact/email".into(),
            index_type: IndexType::Hash,
            unique: true,
        };
        let json = serde_json::to_string(&def).unwrap();
        assert!(json.contains("\"hash\""));
        assert!(json.contains("\"/contact/email\""));
    }
}
