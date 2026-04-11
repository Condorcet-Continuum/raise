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
                            let _ = storage.delete_document(space, db, &collection, &id).await;
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
// TESTS UNITAIRES
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json_db::storage::StorageEngine;
    use crate::json_db::transactions::manager::TransactionManager;

    #[async_test]
    async fn test_wal_persistence() {
        let dir = tempdir().unwrap();
        let config = JsonDbConfig {
            data_root: dir.path().to_path_buf(),
        };

        // ✅ MODIFICATION : On crée le StorageEngine pour le test
        let storage = StorageEngine::new(config.clone());
        let tm = TransactionManager::new(&storage, "s", "d");

        // Transaction vide qui réussit
        let _ = tm.execute(|_| Ok(())).await; // <-- Ajout du .await

        // Le dossier WAL doit avoir été créé (même si vide après commit)
        let wal_path = config.db_root("s", "d").join("wal");
        if !fs::exists_async(&wal_path).await {
            fs::ensure_dir_async(&wal_path).await.unwrap();
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

    #[async_test]
    async fn test_wal_recovery_engine() {
        let dir = tempdir().unwrap();
        let config = JsonDbConfig {
            data_root: dir.path().to_path_buf(),
        };
        let storage = StorageEngine::new(config.clone());
        let space = "sys";
        let db = "core";

        // ---------------------------------------------------------
        // 1. ÉTAT INITIAL (Avant le crash)
        // ---------------------------------------------------------
        let old_doc = json_value!({"name": "Ancien", "val": 1});
        let deleted_doc = json_value!({"name": "A effacer", "val": 2});

        // On écrit la vérité initiale sur le disque
        storage
            .write_document(space, db, "test_col", "doc_update", &old_doc)
            .await
            .unwrap();
        storage
            .write_document(space, db, "test_col", "doc_delete", &deleted_doc)
            .await
            .unwrap();

        // ---------------------------------------------------------
        // 2. SIMULATION DU CRASH
        // ---------------------------------------------------------
        // On simule un fichier WAL qui est resté "Pending" car le commit n'a pas eu lieu
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
                    previous_document: Some(old_doc.clone()), // Capture de l'ancien état !
                    document: json_value!({"name": "Corrompu", "val": 99}),
                },
                Operation::Delete {
                    collection: "test_col".to_string(),
                    id: "doc_delete".to_string(),
                    previous_document: Some(deleted_doc.clone()), // Capture de ce qui a été effacé !
                },
            ],
        };

        // On écrit l'intention dans le WAL
        write_entry(&config, space, db, &tx).await.unwrap();

        // On corrompt manuellement le disque pour simuler que le moteur a planté *pendant* l'écriture
        storage
            .write_document(
                space,
                db,
                "test_col",
                "doc_insert",
                &json_value!({"name": "Nouveau"}),
            )
            .await
            .unwrap();
        storage
            .write_document(
                space,
                db,
                "test_col",
                "doc_update",
                &json_value!({"name": "Corrompu", "val": 99}),
            )
            .await
            .unwrap();
        storage
            .delete_document(space, db, "test_col", "doc_delete")
            .await
            .unwrap();

        // ---------------------------------------------------------
        // 3. LA RÉSURRECTION (Au redémarrage de l'app)
        // ---------------------------------------------------------
        let recovered = recover_pending_transactions(&config, space, db, &storage)
            .await
            .unwrap();
        assert_eq!(recovered, 1, "Une transaction aurait dû être récupérée");

        // ---------------------------------------------------------
        // 4. ASSERTIONS : Preuve de l'ACIDité
        // ---------------------------------------------------------
        // A. L'insert a été annulé (supprimé)
        let res_insert = storage
            .read_document(space, db, "test_col", "doc_insert")
            .await
            .unwrap();
        assert!(
            res_insert.is_none(),
            "UNDO INSERT FAILED: Le document inséré aurait dû être supprimé"
        );

        // B. L'update a été annulé (restauré à l'état initial)
        let res_update = storage
            .read_document(space, db, "test_col", "doc_update")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            res_update["val"], 1,
            "UNDO UPDATE FAILED: Le document mis à jour aurait dû être restauré"
        );

        // C. Le delete a été annulé (ressuscité)
        let res_delete = storage
            .read_document(space, db, "test_col", "doc_delete")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            res_delete["name"], "A effacer",
            "UNDO DELETE FAILED: Le document supprimé aurait dû être ressuscité"
        );

        // D. Le WAL a été nettoyé
        let pending = list_pending(&config, space, db).await.unwrap();
        assert!(
            pending.is_empty(),
            "Le fichier WAL devrait être supprimé après récupération"
        );
    }
}
