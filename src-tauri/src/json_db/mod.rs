// FICHIER : src-tauri/src/json_db/mod.rs

pub mod collections;
pub mod indexes;
pub mod jsonld;
pub mod migrations;
pub mod query;
pub mod schema;
pub mod storage;
pub mod transactions;
// CORRECTION : On retire #[cfg(test)] pour que les tests d'intégration (dossier tests/) puissent l'utiliser.
pub mod test_utils;
