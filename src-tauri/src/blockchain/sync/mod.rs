// src-tauri/src/blockchain/sync/mod.rs

/// Logique de réconciliation des chaînes entre pairs.
pub mod engine;

/// Gestionnaire d'états de synchronisation (Idle, Syncing, UpToDate).
pub mod state;

pub mod delta;

pub use engine::SyncEngine;
pub use state::SyncStatus;
