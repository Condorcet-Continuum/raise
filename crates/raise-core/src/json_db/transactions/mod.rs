// FICHIER : src-tauri/src/json_db/transactions/mod.rs

use crate::utils::prelude::*;

pub mod lock_manager;
pub mod manager;
pub mod wal;

// --- API PUBLIQUE (Haut Niveau) ---
#[derive(Debug, Clone, Serializable, Deserializable)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum TransactionRequest {
    Insert {
        collection: String,
        id: Option<String>,
        document: JsonValue,
    },
    Update {
        collection: String,
        id: Option<String>,
        handle: Option<String>,
        document: JsonValue,
    },
    Upsert {
        collection: String,
        id: Option<String>,
        handle: Option<String>,
        document: JsonValue,
    },
    Delete {
        collection: String,
        id: String,
    },
    InsertFrom {
        collection: String,
        path: String,
    },
    UpdateFrom {
        collection: String,
        path: String,
    },
    UpsertFrom {
        collection: String,
        path: String,
    },
}

// --- INTERNE (Bas Niveau / ACID) ---
#[derive(Debug, Clone, Serializable, Deserializable)]
pub struct Transaction {
    pub id: String,
    pub operations: Vec<Operation>,
}

impl Transaction {
    pub fn new() -> Self {
        Self {
            id: UniqueId::new_v4().to_string(),
            operations: Vec::new(),
        }
    }

    pub fn add_insert(&mut self, collection: &str, id: &str, doc: JsonValue) {
        self.operations.push(Operation::Insert {
            collection: collection.to_string(),
            id: id.to_string(),
            document: doc,
        });
    }
}

impl Default for Transaction {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Serializable, Deserializable)]
pub enum Operation {
    Insert {
        collection: String,
        id: String,
        document: JsonValue,
    },
    Update {
        collection: String,
        id: String,
        previous_document: Option<JsonValue>,
        document: JsonValue,
    },
    Delete {
        collection: String,
        id: String,
        previous_document: Option<JsonValue>,
    },
}

#[derive(Debug, Clone, Serializable, Deserializable)]
pub enum TransactionStatus {
    Pending,
    Committed,
    Rollback,
}

#[derive(Debug, Clone, Serializable, Deserializable)]
pub struct TransactionLog {
    pub id: String,
    pub status: TransactionStatus,
    pub operations: Vec<Operation>,
    pub timestamp: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serialization() {
        let req = TransactionRequest::Insert {
            collection: "users".into(),
            id: None,
            document: json_value!({"a": 1}),
        };
        let s = json::serialize_to_string(&req).unwrap();
        assert!(s.contains("\"type\":\"insert\""));
    }
}
