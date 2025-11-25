//! Module de gestion de base de données JSON
//!
//! Fonctionnalités:
//! - Collections avec schémas JSON Schema
//! - Support JSON-LD pour contexte sémantique
//! - Indexes pour requêtes rapides
//! - Transactions ACID
//! - Migrations de schémas

use anyhow::Result;
use std::path::Path;

// --- Imports qui définissent la structure JsonDb (Résout L15/L17) ---
// Ces lignes sont nécessaires pour la définition de la struct JsonDb.
use self::collections::manager::CollectionsManager; // Ligne 15
use self::storage::JsonDbConfig;

// Déclarations des modules
pub mod collections;
pub mod indexes;
pub mod jsonld;
pub mod migrations;
pub mod query;
pub mod schema;
pub mod storage;
pub mod transactions;

#[doc(hidden)]
pub mod test_utils;
// ===========================================================================
// STRUCTURE PRINCIPALE : JsonDb (Définition UNIQUE - L35)
// ===========================================================================

/// La structure principale de la base de données JSON.
#[derive(Debug, Clone)]
pub struct JsonDb {
    // Définition du type (L35)
    config: JsonDbConfig,
}

impl JsonDb {
    /// Crée une nouvelle instance de JsonDb en chargeant la configuration.
    pub fn new(repo_root: impl AsRef<Path>) -> Result<Self> {
        let config = JsonDbConfig::from_env(repo_root)?;
        Ok(Self { config })
    }

    /// Crée un manager lié à un espace et une base de données spécifiques.
    pub fn collections_manager<'a>(&'a self, space: &str, db: &str) -> CollectionsManager<'a> {
        CollectionsManager::new(&self.config, space, db)
    }
}

// ===========================================================================
// RÉ-EXPORTATIONS PUBLIQUES (API Facade)
// ===========================================================================

// 💡 EXPORT 2: Types de requête (Résout L64)
pub use self::query::{QueryEngine, QueryInput, QueryResult};

// On exporte uniquement les types non conflictuels :
pub use self::jsonld::JsonLdContext;
pub use self::schema::SchemaValidator;
pub use self::transactions::TransactionManager;
// Note : StorageEngine et CollectionsManager sont désormais accessibles via leurs chemins complets.
pub use self::storage::StorageEngine;
