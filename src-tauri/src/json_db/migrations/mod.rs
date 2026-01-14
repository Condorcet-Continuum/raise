// FICHIER : src-tauri/src/json_db/migrations/mod.rs

//! Système de migrations de schémas

pub mod migrator;
pub mod version;

use serde::{Deserialize, Serialize};

pub use migrator::Migrator;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Migration {
    pub id: String,
    pub version: String,
    pub description: String,
    pub up: Vec<MigrationStep>,
    pub down: Vec<MigrationStep>,
    pub applied_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")] // Utilisation d'un tag pour le polymorphisme JSON
pub enum MigrationStep {
    CreateCollection {
        name: String,
        schema: serde_json::Value,
    },
    DropCollection {
        name: String,
    },
    AddField {
        collection: String,
        field: String,
        default: Option<serde_json::Value>,
    },
    RemoveField {
        collection: String,
        field: String,
    },
    RenameField {
        collection: String,
        old_name: String,
        new_name: String,
    },
    CreateIndex {
        collection: String,
        fields: Vec<String>,
    },
    DropIndex {
        collection: String,
        name: String,
    },
}

// ============================================================================
// TESTS UNITAIRES
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_migration_step_serialization() {
        let step = MigrationStep::AddField {
            collection: "users".to_string(),
            field: "active".to_string(),
            default: Some(json!(true)),
        };

        let serialized = serde_json::to_string(&step).unwrap();
        // Vérifie la présence du tag "type" ajouté par #[serde(tag = "type")]
        assert!(serialized.contains("\"type\":\"AddField\""));
        assert!(serialized.contains("\"field\":\"active\""));
    }
}
