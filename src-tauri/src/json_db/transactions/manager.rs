use crate::json_db::collections::collection;
use crate::json_db::indexes;
use crate::json_db::storage::file_storage::DbItemRef; // <--- Import nécessaire
use crate::json_db::storage::{file_storage, JsonDbConfig};
use anyhow::Result;

use super::transaction::ActiveTransaction;
use super::wal::WalManager;
use super::{Operation, TransactionStatus};

/// Orchestrateur de transactions ACID
pub struct TransactionManager<'a> {
    cfg: &'a JsonDbConfig,
    space: String,
    db: String,
    wal: WalManager,
}

impl<'a> TransactionManager<'a> {
    pub fn new(cfg: &'a JsonDbConfig, space: &str, db: &str) -> Self {
        Self {
            cfg,
            space: space.to_string(),
            db: db.to_string(),
            wal: WalManager::new(cfg, space, db),
        }
    }

    /// Exécute un bloc transactionnel.
    pub fn execute<F>(&self, f: F) -> Result<()>
    where
        F: FnOnce(&mut ActiveTransaction) -> Result<()>,
    {
        // 1. Staging (Mémoire)
        let mut tx = ActiveTransaction::new();
        f(&mut tx)?;

        if tx.is_empty() {
            return Ok(());
        }

        // 2. Commit (Disque)
        self.commit(tx)
    }

    fn commit(&self, tx: ActiveTransaction) -> Result<()> {
        // A. Durabilité : Écriture WAL
        let record = tx.to_record(TransactionStatus::Committed);
        self.wal.log_transaction(&record)?;

        // B. Chargement de l'index principal (_system.json) pour mise à jour groupée
        // Cela évite de relire/réécrire le fichier pour chaque opération
        let mut db_index = file_storage::read_index(self.cfg, &self.space, &self.db)?;

        // C. Application : Écriture réelle + Mise à jour Index
        for op in &record.operations {
            match op {
                Operation::Insert {
                    collection,
                    id,
                    document,
                } => {
                    // 1. Écriture physique du fichier
                    collection::persist_insert(
                        self.cfg,
                        &self.space,
                        &self.db,
                        collection,
                        document,
                    )?;

                    // 2. Mise à jour des index de recherche
                    indexes::update_indexes(
                        self.cfg,
                        &self.space,
                        &self.db,
                        collection,
                        id,
                        None,
                        Some(document),
                    )?;

                    // 3. Mise à jour de l'index principal (Liste des items)
                    if let Some(coll_def) = db_index.collections.get_mut(collection) {
                        let file_name = format!("{}.json", id);
                        // Évite les doublons si le fichier existait déjà (cas de réparation)
                        if !coll_def.items.iter().any(|i| i.file == file_name) {
                            coll_def.items.push(DbItemRef { file: file_name });
                        }
                    }
                }
                Operation::Update {
                    collection,
                    id,
                    old_document,
                    new_document,
                } => {
                    // 1. Remplacement physique
                    collection::persist_update(
                        self.cfg,
                        &self.space,
                        &self.db,
                        collection,
                        new_document,
                    )?;

                    // 2. Mise à jour des index de recherche
                    indexes::update_indexes(
                        self.cfg,
                        &self.space,
                        &self.db,
                        collection,
                        id,
                        old_document.as_ref(),
                        Some(new_document),
                    )?;

                    // Pas de changement dans _system.json pour un update (le fichier existe déjà)
                }
                Operation::Delete {
                    collection,
                    id,
                    old_document,
                } => {
                    // 1. Suppression physique
                    collection::delete_document(self.cfg, &self.space, &self.db, collection, id)?;

                    // 2. Nettoyage des index de recherche
                    if let Some(old) = old_document {
                        indexes::update_indexes(
                            self.cfg,
                            &self.space,
                            &self.db,
                            collection,
                            id,
                            Some(old),
                            None,
                        )?;
                    }

                    // 3. Suppression de l'entrée dans l'index principal
                    if let Some(coll_def) = db_index.collections.get_mut(collection) {
                        let file_name = format!("{}.json", id);
                        coll_def.items.retain(|i| i.file != file_name);
                    }
                }
            }
        }

        // D. Sauvegarde finale de l'index principal mis à jour
        file_storage::write_index(self.cfg, &self.space, &self.db, &db_index)?;

        Ok(())
    }
}
