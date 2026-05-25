// FICHIER : src-tauri/src/json_db/indexes/mod.rs

pub mod btree;
pub mod driver;
pub mod hash;
pub mod manager;
pub mod paths;
pub mod text;

use crate::utils::prelude::*;

pub use manager::IndexManager;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serializable, Deserializable)]
#[serde(rename_all = "lowercase")]
pub enum IndexType {
    Hash,
    BTree,
    Text,
}

#[derive(Debug, Clone, Serializable, Deserializable)]
pub struct IndexDefinition {
    pub name: String,
    pub field_path: String,
    pub index_type: IndexType,
    pub unique: bool,
}

#[derive(Debug, Clone, Serializable, Deserializable)]
pub struct IndexRecord {
    pub key: String,
    pub document_id: String,
}

// ============================================================================
// TESTS UNITAIRES
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_index_type_serialization() {
        // Vérifie que les enums sont sérialisés en minuscule ("hash" et pas "Hash")
        let t1 = IndexType::Hash;
        assert_eq!(json::serialize_to_value(t1).unwrap(), json_value!("hash")); // Corrigé

        let t2 = IndexType::BTree;
        assert_eq!(json::serialize_to_value(t2).unwrap(), json_value!("btree"));
        // Corrigé
    }

    #[test]
    fn test_index_definition_structure() {
        let def = IndexDefinition {
            name: "email".to_string(),
            field_path: "/contact/email".to_string(),
            index_type: IndexType::Hash,
            unique: true,
        };

        let json = json::serialize_to_string(&def).unwrap();
        // On vérifie que le json contient bien "hash" en minuscule
        assert!(json.contains("\"hash\""));

        let loaded: IndexDefinition = json::deserialize_from_str(&json).unwrap();
        assert_eq!(loaded.index_type, IndexType::Hash);
    }
}
