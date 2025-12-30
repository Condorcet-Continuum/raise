//! Système de migrations de schémas

pub mod migrator;
pub mod version;

use serde::{Deserialize, Serialize};

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
