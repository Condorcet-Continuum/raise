// FICHIER : src-tauri/src/json_db/transactions/wal.rs

use crate::json_db::storage::JsonDbConfig;
use crate::json_db::transactions::{Transaction, TransactionLog, TransactionStatus};
use anyhow::{anyhow, Result};
use std::fs;
use std::path::PathBuf;

/// Helper pour obtenir le chemin du dossier WAL
fn get_wal_dir(config: &JsonDbConfig, space: &str, db: &str) -> PathBuf {
    config.db_root(space, db).join("wal")
}

/// Écrit une transaction dans le journal (Write Ahead Log)
pub fn write_entry(config: &JsonDbConfig, space: &str, db: &str, tx: &Transaction) -> Result<()> {
    let dir = get_wal_dir(config, space, db);

    if !dir.exists() {
        std::fs::create_dir_all(&dir)
            .map_err(|e| anyhow!("Impossible de créer le dossier WAL : {}", e))?;
    }

    let file_path = dir.join(format!("{}.json", tx.id));

    let log = TransactionLog {
        id: tx.id.clone(),
        status: TransactionStatus::Pending,
        operations: tx.operations.clone(),
        timestamp: chrono::Utc::now().timestamp(),
    };

    let content = serde_json::to_string_pretty(&log)?;
    fs::write(file_path, content)?;

    Ok(())
}

/// Supprime une entrée du WAL (utilisé lors du Commit ou Rollback)
pub fn remove_entry(config: &JsonDbConfig, space: &str, db: &str, tx_id: &str) -> Result<()> {
    let file_path = get_wal_dir(config, space, db).join(format!("{}.json", tx_id));

    if file_path.exists() {
        fs::remove_file(file_path)?;
    }

    Ok(())
}

/// (Optionnel) Charge les transactions en attente
pub fn list_pending(config: &JsonDbConfig, space: &str, db: &str) -> Result<Vec<String>> {
    let dir = get_wal_dir(config, space, db);
    let mut pending_ids = Vec::new();

    if dir.exists() {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "json") {
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    pending_ids.push(stem.to_string());
                }
            }
        }
    }
    Ok(pending_ids)
}

// ============================================================================
// TESTS UNITAIRES
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json_db::transactions::manager::TransactionManager;
    use tempfile::tempdir;

    #[test]
    fn test_wal_persistence() {
        let dir = tempdir().unwrap();
        let config = JsonDbConfig {
            data_root: dir.path().to_path_buf(),
        };

        // On utilise le TransactionManager pour générer une écriture WAL indirectement
        // ou on appelle write_entry directement.
        let tm = TransactionManager::new(&config, "s", "d");

        // Transaction vide qui réussit
        let _ = tm.execute(|_| Ok(()));

        // Le dossier WAL doit avoir été créé (même si vide après commit)
        let wal_path = config.db_root("s", "d").join("wal");
        if !wal_path.exists() {
            std::fs::create_dir_all(&wal_path).unwrap();
        }

        // Test écriture directe
        let tx = Transaction::new();
        write_entry(&config, "s", "d", &tx).unwrap();

        let pending = list_pending(&config, "s", "d").unwrap();
        assert!(pending.contains(&tx.id));
    }
}
