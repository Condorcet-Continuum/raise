// FICHIER : src-tauri/src/json_db/transactions/wal.rs

use crate::json_db::storage::{JsonDbConfig, StorageEngine};
use crate::json_db::transactions::{Operation, Transaction, TransactionLog, TransactionStatus};

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
) -> RaiseResult<()> {
    let dir = get_wal_dir(config, space, db);

    fs::ensure_dir_async(&dir).await?;

    let file_path = dir.join(format!("{}.json", tx.id));

    let log = TransactionLog {
        id: tx.id.clone(),
        status: TransactionStatus::Pending,
        operations: tx.operations.clone(),
        timestamp: UtcClock::now().timestamp(),
    };

    fs::write_json_atomic_async(&file_path, &log).await?;

    Ok(())
}

/// Supprime une entrée du WAL (utilisé lors du Commit ou Rollback)
pub async fn remove_entry(
    config: &JsonDbConfig,
    space: &str,
    db: &str,
    tx_id: &str,
) -> RaiseResult<()> {
    let file_path = get_wal_dir(config, space, db).join(format!("{}.json", tx_id));

    if fs::exists_async(&file_path).await {
        fs::remove_file_async(&file_path).await?;
    }

    Ok(())
}

/// (Optionnel) Charge les transactions en attente
pub async fn list_pending(
    config: &JsonDbConfig,
    space: &str,
    db: &str,
) -> RaiseResult<Vec<String>> {
    let dir = get_wal_dir(config, space, db);
    let mut pending_ids = Vec::new();

    if fs::exists_async(&dir).await {
        let mut entries = fs::read_dir_async(&dir).await?;
        while let Some(entry) = match entries.next_entry().await {
            Ok(e) => e,
            Err(e) => raise_error!(
                "ERR_FS_SCAN_ITERATION_FAIL",
                error = e,
                context = json_value!({
                    "directory": dir,
                    "action": "collect_pending_ids"
                })
            ),
        } {
            let path = entry.path();

            // Filtrage des fichiers JSON
            if path.extension().is_some_and(|ext| ext == "json") {
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    pending_ids.push(stem.to_string());
                }
            }
        }
    }
    Ok(pending_ids)
}

/// 🎯 MOTEUR DE RECOVERY : Exécute la récupération après un crash
/// À appeler obligatoirement au démarrage du backend, avant d'accepter la moindre requête.
pub async fn recover_pending_transactions(
    config: &JsonDbConfig,
    space: &str,
    db: &str,
    storage: &StorageEngine,
) -> RaiseResult<usize> {
    // 1. On liste tous les fichiers TX qui sont restés sur le disque (Preuve d'un crash)
    let pending_ids = list_pending(config, space, db).await?;
    let mut recovered_count = 0;

    for tx_id in pending_ids {
        let file_path = get_wal_dir(config, space, db).join(format!("{}.json", tx_id));

        if let Ok(content) = fs::read_to_string_async(&file_path).await {
            if let Ok(log) = json::deserialize_from_str::<TransactionLog>(&content) {
                #[cfg(debug_assertions)]
                println!(
                    "⚠️ [WAL] Crash détecté ! Restauration (Rollback) de la transaction {}...",
                    tx_id
                );

                // 2. On annule les opérations À L'ENVERS (LIFO - Last In, First Out)
                for op in log.operations.into_iter().rev() {
                    match op {
                        Operation::Insert { collection, id, .. } => {
                            // On supprime le fichier qui a potentiellement été écrit
                            if let Err(e) =
                                storage.delete_document(space, db, &collection, &id).await
                            {
                                raise_error!(
                                    "ERR_WAL_RECOVERY_IO",
                                    error = format!("Impossible d'annuler l'insertion : {}", e),
                                    context = json_value!({"collection": collection, "id": id, "tx_id": tx_id})
                                );
                            }
                        }
                        Operation::Update {
                            collection,
                            id,
                            previous_document,
                            ..
                        } => {
                            // On restaure l'ancien document (Undo)
                            if let Some(old_doc) = previous_document {
                                let _ = storage
                                    .write_document(space, db, &collection, &id, &old_doc)
                                    .await;
                            }
                        }
                        Operation::Delete {
                            collection,
                            id,
                            previous_document,
                        } => {
                            // On ressuscite le document qui a été effacé à tort
                            if let Some(old_doc) = previous_document {
                                let _ = storage
                                    .write_document(space, db, &collection, &id, &old_doc)
                                    .await;
                            }
                        }
                    }
                }
                recovered_count += 1;
            }
        }
        // 3. Le nettoyage est terminé, la base est de nouveau cohérente.
        // On supprime l'entrée du WAL pour ne pas la rejouer au prochain démarrage.
        fs::remove_file_async(&file_path).await.ok();
    }

    Ok(recovered_count)
}

// ============================================================================
// TESTS UNITAIRES (Pattern Wrapper : Zéro Unwrap, Retour Unitaire)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::testing::DbSandbox;

    #[async_test]
    async fn test_wal_persistence() {
        // 🎯 L'encapsulation permet de conserver '?' sans contredire la macro #[async_test]
        async fn run() -> RaiseResult<()> {
            let sandbox = DbSandbox::new().await;
            let config = sandbox.storage.config.clone();
            let space = "s";
            let db = "d";

            let tx = Transaction::new();
            write_entry(&config, space, db, &tx).await?;

            let pending = list_pending(&config, space, db).await?;

            if !pending.contains(&tx.id) {
                raise_error!(
                    "ERR_TEST_ASSERTION_FAILED",
                    error = "La transaction n'a pas été trouvée dans le WAL après écriture."
                );
            }

            Ok(())
        }

        // Exécution et interception au niveau du Test Runner
        if let Err(e) = run().await {
            panic!("❌ Échec du test 'test_wal_persistence' : {}", e);
        }
    }

    #[async_test]
    async fn test_wal_recovery_engine() {
        async fn run() -> RaiseResult<()> {
            let sandbox = DbSandbox::new().await;
            let storage = &sandbox.storage;
            let config = storage.config.clone();
            let space = "sys";
            let db = "core";

            // 1. ÉTAT INITIAL (Avant le crash)
            let old_doc = json_value!({"name": "Ancien", "val": 1});
            let deleted_doc = json_value!({"name": "A effacer", "val": 2});

            storage
                .write_document(space, db, "test_col", "doc_update", &old_doc)
                .await?;
            storage
                .write_document(space, db, "test_col", "doc_delete", &deleted_doc)
                .await?;

            // 2. SIMULATION DU CRASH
            let tx = Transaction {
                id: "tx-crash-123".to_string(),
                operations: vec![
                    Operation::Insert {
                        collection: "test_col".to_string(),
                        id: "doc_insert".to_string(),
                        document: json_value!({"name": "Nouveau"}),
                    },
                    Operation::Update {
                        collection: "test_col".to_string(),
                        id: "doc_update".to_string(),
                        previous_document: Some(old_doc.clone()),
                        document: json_value!({"name": "Corrompu", "val": 99}),
                    },
                    Operation::Delete {
                        collection: "test_col".to_string(),
                        id: "doc_delete".to_string(),
                        previous_document: Some(deleted_doc.clone()),
                    },
                ],
            };

            write_entry(&config, space, db, &tx).await?;

            // Corruption manuelle simulant le crash I/O
            storage
                .write_document(
                    space,
                    db,
                    "test_col",
                    "doc_insert",
                    &json_value!({"name": "Nouveau"}),
                )
                .await?;
            storage
                .write_document(
                    space,
                    db,
                    "test_col",
                    "doc_update",
                    &json_value!({"name": "Corrompu", "val": 99}),
                )
                .await?;
            storage
                .delete_document(space, db, "test_col", "doc_delete")
                .await?;

            // 3. LA RÉSURRECTION
            let recovered = recover_pending_transactions(&config, space, db, storage).await?;

            if recovered != 1 {
                raise_error!(
                    "ERR_TEST_ASSERTION_FAILED",
                    error = "Une transaction aurait dû être récupérée"
                );
            }

            // 4. ASSERTIONS : Preuve de l'ACIDité
            let res_insert = storage
                .read_document(space, db, "test_col", "doc_insert")
                .await?;
            if res_insert.is_some() {
                raise_error!("ERR_TEST_ASSERTION_FAILED", error = "UNDO INSERT FAILED");
            }

            let res_update = storage
                .read_document(space, db, "test_col", "doc_update")
                .await?;
            if res_update.map_or(true, |d| d["val"] != 1) {
                raise_error!("ERR_TEST_ASSERTION_FAILED", error = "UNDO UPDATE FAILED");
            }

            let res_delete = storage
                .read_document(space, db, "test_col", "doc_delete")
                .await?;
            if res_delete.map_or(true, |d| d["name"] != "A effacer") {
                raise_error!("ERR_TEST_ASSERTION_FAILED", error = "UNDO DELETE FAILED");
            }

            let pending = list_pending(&config, space, db).await?;
            if !pending.is_empty() {
                raise_error!(
                    "ERR_TEST_ASSERTION_FAILED",
                    error = "Le fichier WAL devrait être supprimé après récupération"
                );
            }

            Ok(())
        }

        // Exécution et interception au niveau du Test Runner
        if let Err(e) = run().await {
            panic!("❌ Échec du test 'test_wal_recovery_engine' : {}", e);
        }
    }
}
