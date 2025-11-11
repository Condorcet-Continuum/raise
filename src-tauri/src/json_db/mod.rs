//! Module de gestion de base de données JSON
//!
//! Fonctionnalités:
//! - Collections avec schémas JSON Schema
//! - Support JSON-LD pour contexte sémantique
//! - Indexes pour requêtes rapides
//! - Transactions ACID
//! - Migrations de schémas

pub mod collections;
pub mod indexes;
pub mod jsonld;
pub mod migrations;
pub mod query;
pub mod schema;
pub mod storage;
pub mod transactions;

//pub use collections::CollectionManager;
pub use jsonld::JsonLdContext;
pub use query::QueryEngine;
pub use schema::SchemaValidator;
pub use storage::StorageEngine;
pub use transactions::TransactionManager;
