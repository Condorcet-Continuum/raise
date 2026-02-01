// src-tauri/src/blockchain/p2p/mod.rs

/// Gestion du comportement réseau (Swarm, protocoles).
pub mod behavior;

/// Définition des types de messages et de la grammaire réseau.
pub mod protocol;

/// Logique de gestion du Swarm (connexions, événements).
pub mod swarm;

pub mod vpn;

// Réexportation pour simplifier l'accès
pub use protocol::{ArcadiaNetMessage, ArcadiaResponse};
