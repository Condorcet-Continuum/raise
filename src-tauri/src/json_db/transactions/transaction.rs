use super::{Operation, TransactionRecord, TransactionStatus};
use chrono::Utc;
use serde_json::Value;
use uuid::Uuid;

/// Une transaction active en cours de construction (Staging Area).
/// Elle vit uniquement en mémoire le temps du bloc `execute`.
pub struct ActiveTransaction {
    pub id: String,
    pub operations: Vec<Operation>,
    created_at: i64,
}

impl Default for ActiveTransaction {
    fn default() -> Self {
        Self::new()
    }
}

impl ActiveTransaction {
    pub fn new() -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            operations: Vec::new(),
            created_at: Utc::now().timestamp_millis(),
        }
    }

    /// Ajoute une opération d'insertion
    pub fn add_insert(&mut self, collection: &str, id: &str, doc: Value) {
        self.operations.push(Operation::Insert {
            collection: collection.to_string(),
            id: id.to_string(),
            document: doc,
        });
    }

    /// Ajoute une opération de mise à jour
    pub fn add_update(&mut self, collection: &str, id: &str, old: Option<Value>, new: Value) {
        self.operations.push(Operation::Update {
            collection: collection.to_string(),
            id: id.to_string(),
            old_document: old,
            new_document: new,
        });
    }

    /// Ajoute une opération de suppression
    pub fn add_delete(&mut self, collection: &str, id: &str, old: Option<Value>) {
        self.operations.push(Operation::Delete {
            collection: collection.to_string(),
            id: id.to_string(),
            old_document: old,
        });
    }

    /// Vérifie si la transaction est vide
    pub fn is_empty(&self) -> bool {
        self.operations.is_empty()
    }

    /// Convertit la transaction active en enregistrement immuable pour le WAL
    pub fn to_record(&self, status: TransactionStatus) -> TransactionRecord {
        TransactionRecord {
            id: self.id.clone(),
            operations: self.operations.clone(),
            status,
            started_at: self.created_at,
        }
    }
}
