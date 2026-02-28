// FICHIER : src-tauri/tests/json_db_suite.rs
// Module commun (Setup, Helpers, Environnement asynchrone)
#[path = "common/mod.rs"]
mod common;
/*
use raise::json_db::collections::manager::CollectionsManager;
use raise::json_db::storage::{JsonDbConfig, StorageEngine};
use raise::utils::{
    async_recursion,
    error::AnyResult,
    fs::{self, Path, PathBuf},
    Once, // Exporté dans mod.rs
};
*/
// --- DÉCLARATION EXPLICITE DES MODULES ---
// On dit à Rust exactement où trouver chaque fichier dans le sous-dossier

#[path = "json_db_suite/dataset_integration.rs"]
pub mod dataset_integration;

#[path = "json_db_suite/json_db_errors.rs"]
pub mod json_db_errors;

#[path = "json_db_suite/json_db_idempotent.rs"]
pub mod json_db_idempotent;

#[path = "json_db_suite/json_db_integration.rs"]
pub mod json_db_integration;

#[path = "json_db_suite/json_db_lifecycle.rs"]
pub mod json_db_lifecycle;

#[path = "json_db_suite/json_db_query_integration.rs"]
pub mod json_db_query_integration;

#[path = "json_db_suite/json_db_sql.rs"]
pub mod json_db_sql;

#[path = "json_db_suite/json_db_indexes_ops.rs"]
pub mod json_db_indexes_ops;

#[path = "json_db_suite/schema_consistency.rs"]
pub mod schema_consistency;

#[path = "json_db_suite/schema_minimal.rs"]
pub mod schema_minimal;

#[path = "json_db_suite/workunits_x_compute.rs"]
pub mod workunits_x_compute;

#[path = "json_db_suite/integration_suite.rs"]
pub mod integration_suite;
