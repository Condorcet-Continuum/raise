//! Module de gestion de base de données JSON
//! 
//! Fonctionnalités:
//! - Collections avec schémas JSON Schema
//! - Support JSON-LD pour contexte sémantique
//! - Indexes pour requêtes rapides
//! - Transactions ACID
//! - Migrations de schémas

pub mod collections;
pub mod schema;
pub mod jsonld;
pub mod query;
pub mod storage;
pub mod transactions;
pub mod indexes;
pub mod migrations;

pub use collections::CollectionManager;
pub use schema::SchemaValidator;
pub use jsonld::JsonLdContext;
pub use query::QueryEngine;
pub use storage::StorageEngine;
pub use transactions::TransactionManager;
