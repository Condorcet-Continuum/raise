// FICHIER : src-tauri/src/json_db/transactions/mod.rs

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub mod lock_manager;
pub mod manager;
pub mod wal;

#[cfg(test)]
mod tests;

// --- API PUBLIQUE (Haut Niveau) ---
// C'est ce que le CLI ou le Frontend envoie.
// Le Manager va transformer ça en opérations atomiques.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum TransactionRequest {
    Insert {
        collection: String,
        id: Option<String>, // Optionnel (Auto-généré)
        document: Value,
    },
    Update {
        collection: String,
        id: Option<String>,     // Cible par ID
        handle: Option<String>, // OU Cible par Handle
        document: Value,        // Patch à merger
    },
    Delete {
        collection: String,
        id: String,
    },
    InsertFrom {
        collection: String,
        path: String,
    },
}

// --- INTERNE (Bas Niveau / ACID) ---
// C'est ce qui est écrit dans le WAL et exécuté physiquement.
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

    // Helper pour les tests unitaires (C'est ce qu'il manquait !)
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
