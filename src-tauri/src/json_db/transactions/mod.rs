//! Gestion des transactions ACID

pub mod transaction;
pub mod lock_manager;
pub mod wal;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transaction {
    pub id: String,
    pub operations: Vec<Operation>,
    pub status: TransactionStatus,
    pub started_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Operation {
    Insert { collection: String, document: serde_json::Value },
    Update { collection: String, id: String, document: serde_json::Value },
    Delete { collection: String, id: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TransactionStatus {
    Pending,
    Committed,
    Aborted,
}
