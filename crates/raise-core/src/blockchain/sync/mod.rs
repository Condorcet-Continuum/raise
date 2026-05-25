// src-tauri/src/blockchain/sync/mod.rs
//! Sous-module de synchronisation Mentis : Maintient la cohérence de la JSON-DB distribuée.

/// Logique de réconciliation des chaînes entre pairs (Request-Response).
pub mod engine;

/// Calcul des écarts de données (diff) pour la synchronisation optimisée.
pub mod delta;

/// Gestionnaire d'états de synchronisation (Initializing, Syncing, UpToDate).
pub mod state;

// =========================================================================
// FAÇADE DE SYNCHRONISATION
// =========================================================================
// Réexportations stratégiques pour simplifier l'usage depuis Tauri ou le service P2P.

pub use delta::MentisDelta;
pub use engine::SyncEngine;
pub use state::SyncStatus;
