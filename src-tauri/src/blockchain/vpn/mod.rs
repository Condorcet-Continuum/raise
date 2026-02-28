// src-tauri/src/blockchain/vpn/mod.rs

pub mod innernet_client;

// Ré-export des structures définies physiquement dans innernet_client.rs
pub use innernet_client::{InnernetClient, NetworkConfig, NetworkStatus, Peer};
