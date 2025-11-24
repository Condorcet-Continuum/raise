//! CollectionsManager : façade orientée instance (cfg, space, db)
//! - cache le SchemaRegistry
//! - expose des méthodes CRUD cohérentes (avec et sans schéma)
//! - centralise les chemins cibles de collection (dérivés du schéma)
//! - Gère automatiquement la cohérence des INDEXES à chaque écriture

use anyhow::{anyhow, Context, Result};
use serde_json::Value;
use std::sync::RwLock;

use crate::json_db::{
    indexes::{create_collection_indexes, update_indexes},
    schema::{SchemaRegistry, SchemaValidator},
    storage::JsonDbConfig,
};

// Imports des primitives de collection (FS)
use super::collection::create_collection_if_missing;
use super::collection::delete_document as delete_document_fs;
use super::collection::drop_collection as drop_collection_fs;
use super::collection::list_collection_names_fs;
use super::collection::list_document_ids as list_document_ids_fs;
use super::collection::list_documents as list_documents_fs;
use super::collection::read_document as read_document_fs;

// Imports restaurés et utilisés
use super::collection::persist_insert;
use super::collection::persist_update;
use super::collection_from_schema_rel;

/// Manager lié à un couple (space, db)
#[derive(Debug)]
pub struct CollectionsManager<'a> {
    cfg: &'a JsonDbConfig,
    space: String,
    db: String,
    // RwLock pour la mutabilité interne thread-safe
    registry: RwLock<Option<SchemaRegistry>>,
}

impl<'a> CollectionsManager<'a> {
    /// Construit un manager (le registre est lazy, créé au premier usage)
    pub fn new(cfg: &'a JsonDbConfig, space: &str, db: &str) -> Self {
        Self {
            cfg,
            space: space.to_string(),
            db: db.to_string(),
            registry: RwLock::new(None),
        }
    }

    /// (Re)charge explicitement le registre depuis la DB (forces refresh)
    pub fn refresh_registry(&self) -> Result<()> {
        let reg = SchemaRegistry::from_db(self.cfg, &self.space, &self.db)?;

        // Utilisation de write() pour muter
        *self
            .registry
            .write()
            .map_err(|e| anyhow!("RwLock poisoned on write: {}", e))? = Some(reg);

        Ok(())
    }

    /// Helper interne pour s'assurer que le registre est chargé.
    fn ensure_registry_loaded(&self) -> Result<()> {
        // Vérification rapide en lecture
        let is_none = {
            let guard = self
                .registry
                .read()
                .map_err(|e| anyhow!("RwLock poisoned on read: {}", e))?;
            guard.is_none()
        };

        // Si vide, on charge (avec verrou d'écriture)
        if is_none {
            self.refresh_registry()?;
        }
        Ok(())
    }

    /// Construit une URI logique complète depuis un chemin relatif de schéma.
    pub fn schema_uri(&self, schema_rel: &str) -> Result<String> {
        self.ensure_registry_loaded()?;

        let guard = self
            .registry
            .read()
            .map_err(|e| anyhow!("RwLock poisoned: {}", e))?;
        let reg = guard.as_ref().context("Registry should be initialized")?;

        Ok(reg.uri(schema_rel).to_string())
    }

    /// Compile un validator pour `schema_rel`
    fn compile(&self, schema_rel: &str) -> Result<SchemaValidator> {
        self.ensure_registry_loaded()?;

        let guard = self
            .registry
            .read()
            .map_err(|e| anyhow!("RwLock poisoned: {}", e))?;
        let reg = guard.as_ref().context("Registry should be initialized")?;

        let root_uri = reg.uri(schema_rel);
        SchemaValidator::compile_with_registry(&root_uri, reg)
    }

    // ---------------------------
    // Collections (dossiers & indexes)
    // ---------------------------

    /// Vérifie si la collection (et son index) existe, sinon l'initialise.
    /// C'est ici qu'on garantit que `_config.json` est créé.
    fn ensure_collection_ready(&self, collection: &str, schema_rel: &str) -> Result<()> {
        let root = super::collection::collection_root(self.cfg, &self.space, &self.db, collection);

        // Si le dossier collection n'existe pas, on l'initialise complètement
        if !root.exists() {
            create_collection_if_missing(self.cfg, &self.space, &self.db, collection)?;
            // Création de la config d'index par défaut (id)
            create_collection_indexes(self.cfg, &self.space, &self.db, collection, schema_rel)?;
        } else {
            // Si le dossier existe mais pas la config index, on la crée (migration implicite)
            let config_path = root.join("_config.json");
            if !config_path.exists() {
                create_collection_indexes(self.cfg, &self.space, &self.db, collection, schema_rel)?;
            }
        }
        Ok(())
    }

    pub fn create_collection(&self, collection_name: &str) -> Result<()> {
        // Création avec schéma "unknown" par défaut si appelé manuellement
        self.ensure_collection_ready(collection_name, "unknown")
    }

    pub fn drop_collection(&self, collection_name: &str) -> Result<()> {
        drop_collection_fs(self.cfg, &self.space, &self.db, collection_name)
    }

    // ---------------------------
    // Inserts / Updates (avec gestion des Index)
    // ---------------------------

    /// Insert avec schéma :
    /// - x_compute + validate
    /// - ensure collection + index config
    /// - persist FS
    /// - update index
    pub fn insert_with_schema(&self, schema_rel: &str, mut doc: Value) -> Result<Value> {
        let validator = self.compile(schema_rel)?;
        validator.compute_then_validate(&mut doc)?;

        let collection = collection_from_schema_rel(schema_rel);

        // 1. S'assurer que la structure existe
        self.ensure_collection_ready(&collection, schema_rel)?;

        // 2. Persistance fichier (atomique)
        persist_insert(self.cfg, &self.space, &self.db, &collection, &doc)?;

        // 3. Mise à jour des index (nouveau doc uniquement)
        // Note: doc["id"] est garanti par x_compute/validate
        if let Some(id) = doc.get("id").and_then(|v| v.as_str()) {
            update_indexes(
                self.cfg,
                &self.space,
                &self.db,
                &collection,
                id,
                None,       // Pas d'ancien doc
                Some(&doc), // Nouveau doc
            )?;
        }

        Ok(doc)
    }

    /// Insert direct (sans schéma).
    pub fn insert_raw(&self, collection: &str, doc: &Value) -> Result<()> {
        self.ensure_collection_ready(collection, "unknown")?;
        persist_insert(self.cfg, &self.space, &self.db, collection, doc)?;

        // Mise à jour index si ID présent
        if let Some(id) = doc.get("id").and_then(|v| v.as_str()) {
            update_indexes(
                self.cfg,
                &self.space,
                &self.db,
                collection,
                id,
                None,
                Some(doc),
            )?;
        }
        Ok(())
    }

    /// Update avec schéma :
    /// - Lit l'ancien document (pour nettoyer l'index)
    /// - Compute + Validate
    /// - Persist FS
    /// - Update index (remove old keys + add new keys)
    pub fn update_with_schema(&self, schema_rel: &str, mut doc: Value) -> Result<Value> {
        let validator = self.compile(schema_rel)?;
        validator.compute_then_validate(&mut doc)?;

        let collection = collection_from_schema_rel(schema_rel);
        let id = doc
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Document missing id"))?;

        // 1. Lecture de l'ancien document (nécessaire pour update_indexes)
        // On ignore l'erreur si le fichier n'existe pas encore (cas limite),
        // mais persist_update échouera de toute façon après.
        let old_doc = read_document_fs(self.cfg, &self.space, &self.db, &collection, id).ok();

        // 2. Persistance
        persist_update(self.cfg, &self.space, &self.db, &collection, &doc)?;

        // 3. Mise à jour des index
        update_indexes(
            self.cfg,
            &self.space,
            &self.db,
            &collection,
            id,
            old_doc.as_ref(),
            Some(&doc),
        )?;

        Ok(doc)
    }

    /// Update direct (sans schéma).
    pub fn update_raw(&self, collection: &str, doc: &Value) -> Result<()> {
        let id = doc
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Document missing id"))?;

        let old_doc = read_document_fs(self.cfg, &self.space, &self.db, collection, id).ok();

        persist_update(self.cfg, &self.space, &self.db, collection, doc)?;

        update_indexes(
            self.cfg,
            &self.space,
            &self.db,
            collection,
            id,
            old_doc.as_ref(),
            Some(doc),
        )?;

        Ok(())
    }

    // ---------------------------
    // Lecture / Suppression / Listes
    // ---------------------------

    pub fn get(&self, collection: &str, id: &str) -> Result<Value> {
        read_document_fs(self.cfg, &self.space, &self.db, collection, id)
    }

    /// Delete : supprime le fichier et nettoie les index.
    pub fn delete(&self, collection: &str, id: &str) -> Result<()> {
        // 1. Lire le document avant suppression pour l'index
        let old_doc = read_document_fs(self.cfg, &self.space, &self.db, collection, id).ok();

        // 2. Suppression FS
        delete_document_fs(self.cfg, &self.space, &self.db, collection, id)?;

        // 3. Nettoyage index (si le document existait)
        if let Some(doc) = old_doc {
            update_indexes(
                self.cfg,
                &self.space,
                &self.db,
                collection,
                id,
                Some(&doc),
                None, // Pas de nouveau doc = suppression
            )?;
        }

        Ok(())
    }

    pub fn list_ids(&self, collection: &str) -> Result<Vec<String>> {
        list_document_ids_fs(self.cfg, &self.space, &self.db, collection)
    }

    pub fn list_all(&self, collection: &str) -> Result<Vec<Value>> {
        list_documents_fs(self.cfg, &self.space, &self.db, collection)
    }

    // ---------------------------
    // Helpers pratiques
    // ---------------------------

    pub fn list_ids_for_schema(&self, schema_rel: &str) -> Result<Vec<String>> {
        let collection = collection_from_schema_rel(schema_rel);
        self.list_ids(&collection)
    }

    pub fn upsert_with_schema(&self, schema_rel: &str, doc: Value) -> Result<Value> {
        match self.insert_with_schema(schema_rel, doc.clone()) {
            Ok(stored) => Ok(stored),
            Err(_e) => self.update_with_schema(schema_rel, doc),
        }
    }

    pub fn context(&self) -> (&str, &str) {
        (&self.space, &self.db)
    }

    pub fn list_collection_names(&self) -> Result<Vec<String>> {
        list_collection_names_fs(self.cfg, &self.space, &self.db)
    }
}
