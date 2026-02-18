// src-tauri/src/blockchain/fabric/mod.rs

pub mod client;
pub mod config;

// On ré-exporte le client pour l'utiliser facilement dans le reste de l'app
pub use client::FabricClient;
// On ré-exporte la config pour qu'elle soit accessible
pub use config::ConnectionProfile;
