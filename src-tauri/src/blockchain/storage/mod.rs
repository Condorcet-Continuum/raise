// src-tauri/src/blockchain/storage/mod.rs

/// Gestion de la structure de Commit (Mutations, Signatures et Métadonnées).
pub mod commit;

/// Gestion du registre local (Ledger) et du chaînage des blocs.
pub mod chain;

// Réexportation des structures clés pour un usage simplifié dans le reste de Raise
pub use chain::Ledger;
pub use commit::{MentisCommit, Mutation, MutationOp};
