// FICHIER : src-tauri/src/json_db/migrations/mod.rs

//! Système de migrations de schémas
use crate::utils::prelude::*;

pub mod migrator;
pub mod version;

pub use migrator::Migrator;

#[derive(Debug, Clone, Serializable, Deserializable)]
pub struct Migration {
    pub id: String,
    pub version: String,
    pub description: String,
    pub up: Vec<MigrationStep>,
    pub down: Vec<MigrationStep>,
    pub applied_at: Option<String>,
}

#[derive(Debug, Clone, Serializable, Deserializable, PartialEq, Eq)]
#[serde(tag = "type")] // Utilisation d'un tag pour le polymorphisme JSON
pub enum MigrationStep {
    CreateCollection {
        name: String,
        schema: JsonValue,
    },
    DropCollection {
        name: String,
    },
    AddField {
        collection: String,
        field: String,
        default: Option<JsonValue>,
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
    Custom {
        handler: String,
        params: JsonValue,
    },
}

// ============================================================================
// TESTS UNITAIRES
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_migration_step_roundtrip() -> RaiseResult<()> {
        let step = MigrationStep::AddField {
            collection: "users".to_string(),
            field: "active".to_string(),
            default: Some(json_value!(true)),
        };

        // 1. Sérialisation
        let serialized = match json::serialize_to_string(&step) {
            Ok(s) => s,
            Err(e) => raise_error!("ERR_TEST_SERIALIZATION", error = e.to_string()),
        };
        // 🎯 Vérification du tag "type"
        assert!(
            serialized.contains("\"type\":\"AddField\""),
            "Le tag Serde est manquant."
        );

        // 2. Désérialisation (Le test ultime)
        let deserialized: MigrationStep = match json::deserialize_from_str(&serialized) {
            Ok(d) => d,
            Err(e) => raise_error!("ERR_TEST_DESERIALIZATION", error = e.to_string()),
        };

        // 3. Comparaison structurelle (Nécessite PartialEq)
        assert_eq!(
            step, deserialized,
            "L'objet désérialisé diffère de l'original."
        );

        Ok(())
    }
}
