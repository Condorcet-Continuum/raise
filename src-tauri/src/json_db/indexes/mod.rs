// FICHIER : src-tauri/src/json_db/indexes/mod.rs

pub mod btree;
pub mod driver;
pub mod hash;
pub mod manager;
pub mod paths;
pub mod text;

use serde::{Deserialize, Serialize};

pub use manager::IndexManager;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")] // <--- CORRECTION IMPORTANTE ICI
pub enum IndexType {
    /// Index exact (HashMap). Idéal pour les IDs, emails, codes uniques.
    Hash,

    /// Index ordonné (BTree). Idéal pour les dates, nombres, tris (Range).
    BTree,

    /// Index de recherche textuelle (Inverted Index). Pour la recherche de mots-clés.
    Text,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexDefinition {
    pub name: String,
    pub field_path: String,
    pub index_type: IndexType,
    pub unique: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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
    use serde_json::json;

    #[test]
    fn test_index_type_serialization() {
        // Vérifie que les enums sont sérialisés en minuscule ("hash" et pas "Hash")
        let t1 = IndexType::Hash;
        assert_eq!(serde_json::to_value(t1).unwrap(), json!("hash")); // Corrigé

        let t2 = IndexType::BTree;
        assert_eq!(serde_json::to_value(t2).unwrap(), json!("btree")); // Corrigé
    }

    #[test]
    fn test_index_definition_structure() {
        let def = IndexDefinition {
            name: "email".to_string(),
            field_path: "/contact/email".to_string(),
            index_type: IndexType::Hash,
            unique: true,
        };

        let json = serde_json::to_string(&def).unwrap();
        // On vérifie que le json contient bien "hash" en minuscule
        assert!(json.contains("\"hash\""));

        let loaded: IndexDefinition = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.index_type, IndexType::Hash);
    }
}
