// src-tauri/src/blockchain/vpn/mod.rs

pub mod innernet_client;

// Ré-export des structures définies physiquement dans innernet_client.rs
pub use innernet_client::{InnernetClient, NetworkConfig, NetworkStatus, Peer};

// Ré-export de l'erreur depuis la source centrale (error.rs)
// Cela permet d'utiliser `raise::blockchain::vpn::VpnError` si besoin,
// tout en pointant vers la définition unique dans `blockchain::error`.
pub use crate::blockchain::error::VpnError;
