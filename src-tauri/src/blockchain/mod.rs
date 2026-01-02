// src-tauri/src/blockchain/mod.rs
//! Abstraction minimale de la couche Blockchain / VPN pour RAISE.
//!
//! Ici on définit uniquement :
//! - les types de configuration Fabric / réseau,
//! - un client Fabric minimal,
//! - un client Innernet minimal,
//! - la gestion du state Tauri pour Innernet.
//!
//! ⚠️ Aucune dépendance vers `crate::commands` ici :
//! les commandes Tauri (`blockchain_commands.rs`) viendront utiliser ce module,
//! pas l’inverse.

use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, Runtime, State};

/// Configuration Hyperledger Fabric (version simplifiée).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FabricConfig {
    /// Chemin vers le connection profile Fabric (YAML/JSON).
    pub connection_profile: String,
    /// Nom du channel Fabric.
    pub channel: String,
    /// Nom du chaincode par défaut.
    pub chaincode: String,
}

/// Configuration réseau (Mesh / Innernet / endpoint Fabric, etc.).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    /// Nom logique du réseau (ex: "dev-mesh", "prod-mesh").
    pub network_name: String,
    /// Endpoint de l'API Fabric ou du peer principal.
    pub endpoint: String,
}

/// Client Fabric très simplifié. Il encapsule la config et expose
/// des méthodes de haut niveau (ping, invoke, query...).
#[derive(Debug, Clone)]
pub struct FabricClient {
    pub fabric: FabricConfig,
    pub network: NetworkConfig,
}

impl FabricClient {
    pub fn new(fabric: FabricConfig, network: NetworkConfig) -> Self {
        Self { fabric, network }
    }

    /// Méthode de test pour vérifier que tout est câblé.
    pub fn ping(&self) -> String {
        format!(
            "fabric://channel={} endpoint={} cc={}",
            self.fabric.channel, self.network.endpoint, self.fabric.chaincode
        )
    }
}

/// Client Innernet minimal, géré côté state Tauri.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct InnernetClient {
    /// Nom du profil Innernet (ex: "dev", "prod").
    pub profile: String,
}

impl InnernetClient {
    pub fn new(profile: impl Into<String>) -> Self {
        Self {
            profile: profile.into(),
        }
    }

    pub fn status(&self) -> String {
        format!("innernet profile={}", self.profile)
    }
}

// -----------------------------------------------------------------------------
// Intégration avec le state global Tauri
// -----------------------------------------------------------------------------

/// Type stocké dans l'état Tauri pour Innernet.
pub type SharedInnernetClient = Mutex<InnernetClient>;

/// Initialise un client Innernet dans le state Tauri si nécessaire.
///
/// À appeler typiquement au démarrage de l'app ou dans une commande
/// avant d'utiliser `innernet_state(...)`.
pub fn ensure_innernet_state<R: Runtime>(app: &AppHandle<R>, default_profile: impl Into<String>) {
    if app.try_state::<SharedInnernetClient>().is_none() {
        let client = InnernetClient::new(default_profile);
        app.manage(Mutex::new(client));
    }
}

/// Récupère le client Innernet depuis le state Tauri.
pub fn innernet_state<R: Runtime>(app: &AppHandle<R>) -> State<'_, SharedInnernetClient> {
    app.state::<SharedInnernetClient>()
}
