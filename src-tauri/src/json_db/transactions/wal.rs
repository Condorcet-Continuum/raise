use super::TransactionRecord;
use crate::json_db::storage::JsonDbConfig;
use anyhow::{Context, Result};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;

/// Gestionnaire du Write-Ahead Log (WAL).
/// Assure la durabilité des transactions.
pub struct WalManager {
    wal_path: PathBuf,
}

impl WalManager {
    pub fn new(cfg: &JsonDbConfig, space: &str, db: &str) -> Self {
        // Le fichier WAL est stocké à la racine de la DB
        let wal_path = cfg.db_root(space, db).join("_wal.jsonl");
        Self { wal_path }
    }

    /// Écrit une transaction dans le log (append-only).
    /// Cette opération est bloquante et synchronisée sur le disque.
    pub fn log_transaction(&self, tx: &TransactionRecord) -> Result<()> {
        // S'assurer que le dossier parent existe
        if let Some(parent) = self.wal_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.wal_path)
            .with_context(|| format!("Impossible d'ouvrir le WAL: {:?}", self.wal_path))?;

        // Sérialisation sur une seule ligne (JSONL)
        let json = serde_json::to_string(tx)?;
        writeln!(file, "{}", json)?;

        // SYNC: Crucial pour l'ACID (Durabilité)
        file.sync_all()?;

        Ok(())
    }

    /// Nettoie le WAL (à appeler après un succès complet ou au démarrage après recovery).
    /// Pour l'instant, on supprime simplement le fichier.
    pub fn clear_wal(&self) -> Result<()> {
        if self.wal_path.exists() {
            fs::remove_file(&self.wal_path)?;
        }
        Ok(())
    }
}
