// src-tauri/src/blockchain/mod.rs
//! Point d'entrée du module Blockchain.
//!
//! Agit comme une façade pour :
//! 1. La gestion des erreurs unifiée via `error::BlockchainError`.
//! 2. Le client Hyperledger Fabric réel (`fabric`).
//! 3. Le client VPN Innernet réel (`vpn`).
//! 4. La gestion de l'état global (State) pour l'application Tauri.

use std::sync::Mutex;
use tauri::{AppHandle, Manager, Runtime, State};

// Exposition publique des sous-modules
pub mod error;
pub mod fabric;
pub mod vpn;

// --- RÉ-EXPORTS (La vérité est ailleurs) ---

// On ré-exporte le VRAI client Fabric et sa config depuis le sous-module
pub use self::fabric::client::FabricClient;
pub use self::fabric::config::ConnectionProfile;

// On ré-exporte le VRAI client VPN et sa config
pub use self::vpn::innernet_client::{InnernetClient, NetworkConfig as VpnConfig};

// =============================================================================
//  GESTION DES ÉTATS TAURI (SHARED STATE)
// =============================================================================

// Nous utilisons std::sync::Mutex car nos clients sont conçus pour être Clonés (Arc interne).
// Le pattern est : Lock -> Clone -> Drop Lock -> Async Call sur le clone.

/// Type stocké dans l'état Tauri pour Fabric.
pub type SharedFabricClient = Mutex<FabricClient>;

/// Type stocké dans l'état Tauri pour Innernet.
pub type SharedInnernetClient = Mutex<InnernetClient>;

// --- HELPERS D'ACCÈS ---

/// Helper pour récupérer le client Innernet depuis une commande Tauri.
pub fn innernet_state<R: Runtime>(app: &AppHandle<R>) -> State<'_, SharedInnernetClient> {
    app.state::<SharedInnernetClient>()
}

/// Helper pour récupérer le client Fabric depuis une commande Tauri.
pub fn fabric_state<R: Runtime>(app: &AppHandle<R>) -> State<'_, SharedFabricClient> {
    app.state::<SharedFabricClient>()
}

// --- INITIALISATION ---

/// Initialise le client Innernet dans le state Tauri.
pub fn ensure_innernet_state<R: Runtime>(app: &AppHandle<R>, default_profile: impl Into<String>) {
    if app.try_state::<SharedInnernetClient>().is_none() {
        let profile_name = default_profile.into();

        // Initialisation propre de la config
        let vpn_config = VpnConfig {
            name: profile_name.clone(),
            ..Default::default()
        };

        let client = InnernetClient::new(vpn_config);

        tracing::info!(
            "🔒 [Blockchain] Initialisation State Innernet (profil: {})",
            profile_name
        );
        app.manage(Mutex::new(client));
    }
}

// Note: L'initialisation de Fabric se fera généralement plus tard,
// via une commande `fabric_load_profile` qui chargera le fichier YAML,
// donc pas de `ensure_fabric_state` automatique ici pour l'instant.

#[cfg(test)]
mod tests {
    // Plus de tests unitaires ici car la logique est partie dans les sous-modules.
    // Ce fichier ne fait que du "wiring".
}
