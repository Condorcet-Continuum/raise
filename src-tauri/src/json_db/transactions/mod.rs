// FICHIER : src-tauri/src/json_db/transactions/mod.rs

use crate::utils::json::{Deserialize, Serialize, Value};

pub mod lock_manager;
pub mod manager;
pub mod wal;

// --- API PUBLIQUE (Haut Niveau) ---
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum TransactionRequest {
    Insert {
        collection: String,
        id: Option<String>,
        document: Value,
    },
    Update {
        collection: String,
        id: Option<String>,
        handle: Option<String>,
        document: Value,
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
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transaction {
    pub id: String,
    pub operations: Vec<Operation>,
}

impl Transaction {
    pub fn new() -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            operations: Vec::new(),
        }
    }

    pub fn add_insert(&mut self, collection: &str, id: &str, doc: Value) {
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Operation {
    Insert {
        collection: String,
        id: String,
        document: Value,
    },
    Update {
        collection: String,
        id: String,
        document: Value,
    },
    Delete {
        collection: String,
        id: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TransactionStatus {
    Pending,
    Committed,
    Rollback,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionLog {
    pub id: String,
    pub status: TransactionStatus,
    pub operations: Vec<Operation>,
    pub timestamp: i64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::json::{self, json};

    #[test]
    fn test_serialization() {
        let req = TransactionRequest::Insert {
            collection: "users".into(),
            id: None,
            document: json!({"a": 1}),
        };
        let s = json::stringify(&req).unwrap();
        assert!(s.contains("\"type\":\"insert\""));
    }
}
