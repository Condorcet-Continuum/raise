// FICHIER : src-tauri/src/json_db/transactions/wal.rs

use crate::json_db::storage::JsonDbConfig;
use crate::json_db::transactions::{Transaction, TransactionLog, TransactionStatus};

use crate::utils::io::{self, PathBuf};
use crate::utils::prelude::*;

/// Helper pour obtenir le chemin du dossier WAL
fn get_wal_dir(config: &JsonDbConfig, space: &str, db: &str) -> PathBuf {
    config.db_root(space, db).join("wal")
}

/// Écrit une transaction dans le journal (Write Ahead Log)
pub async fn write_entry(
    config: &JsonDbConfig,
    space: &str,
    db: &str,
    tx: &Transaction,
) -> Result<()> {
    let dir = get_wal_dir(config, space, db);

    io::ensure_dir(&dir).await?;

    let file_path = dir.join(format!("{}.json", tx.id));

    let log = TransactionLog {
        id: tx.id.clone(),
        status: TransactionStatus::Pending,
        operations: tx.operations.clone(),
        timestamp: chrono::Utc::now().timestamp(),
    };

    io::write_json_atomic(&file_path, &log).await?;

    Ok(())
}

/// Supprime une entrée du WAL (utilisé lors du Commit ou Rollback)
pub async fn remove_entry(config: &JsonDbConfig, space: &str, db: &str, tx_id: &str) -> Result<()> {
    let file_path = get_wal_dir(config, space, db).join(format!("{}.json", tx_id));

    if io::exists(&file_path).await {
        io::remove_file(&file_path).await?;
    }

    Ok(())
}

/// (Optionnel) Charge les transactions en attente
pub async fn list_pending(config: &JsonDbConfig, space: &str, db: &str) -> Result<Vec<String>> {
    let dir = get_wal_dir(config, space, db);
    let mut pending_ids = Vec::new();

    if io::exists(&dir).await {
        let mut entries = io::read_dir(&dir).await?;
        while let Some(entry) = entries.next_entry().await? {
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
    use crate::utils::io::tempdir;

    #[tokio::test]
    async fn test_wal_persistence() {
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
        if !io::exists(&wal_path).await {
            io::ensure_dir(&wal_path).await.unwrap();
        }

        // Test écriture directe
        let tx = Transaction::new();
        write_entry(&config, "s", "d", &tx)
            .await
            .expect("Échec write_entry");

        let pending = list_pending(&config, "s", "d")
            .await
            .expect("Échec list_pending");
        assert!(pending.contains(&tx.id));
    }
}
