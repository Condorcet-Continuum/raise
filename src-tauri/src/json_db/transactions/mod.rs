//! Gestion des transactions ACID
//! Définit les types de données partagés pour les transactions.

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub mod lock_manager;
pub mod manager;
pub mod transaction;
pub mod wal;

// 💡 CORRECTION ICI : On ré-exporte le manager pour qu'il soit accessible via json_db::transactions::TransactionManager
pub use manager::TransactionManager;

/// Une opération atomique sur une collection
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Operation {
    Insert {
        collection: String,
        id: String,
        document: Value,
    },
    Update {
        collection: String,
        id: String,
        old_document: Option<Value>, // Pour rollback
        new_document: Value,
    },
    Delete {
        collection: String,
        id: String,
        old_document: Option<Value>, // Pour rollback
    },
}

/// L'enregistrement complet d'une transaction (ce qui est écrit dans le WAL)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionRecord {
    pub id: String,
    pub operations: Vec<Operation>,
    pub status: TransactionStatus,
    pub started_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum TransactionStatus {
    Pending,
    Committed,
    RolledBack,
}

#[cfg(test)]
mod tests;
