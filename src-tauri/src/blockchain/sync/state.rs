// src-tauri/src/blockchain/sync/state.rs
//! État de synchronisation du nœud Mentis (State Machine).

use crate::utils::prelude::*;

/// Représente l'état actuel du nœud dans le réseau P2P.
#[derive(Debug, Serializable, Deserializable, Clone, PartialEq)]
pub enum SyncStatus {
    /// Le nœud vient de démarrer et cherche d'autres pairs (Kademlia Bootstrap).
    Initializing,
    /// Le nœud est en train de télécharger et d'appliquer des commits manquants.
    Syncing { progress: f32, target_hash: String },
    /// Le nœud est parfaitement à jour avec le quorum du réseau.
    UpToDate,
    /// Erreur critique empêchant la synchronisation (ex: fork non résolu, corruption).
    Error(String),
}

/// Implémentation par défaut pour faciliter l'initialisation au lancement de l'application.
impl Default for SyncStatus {
    fn default() -> Self {
        Self::Initializing
    }
}

// =========================================================================
// TESTS UNITAIRES
// =========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sync_status_default() {
        assert_eq!(SyncStatus::default(), SyncStatus::Initializing);
    }

    #[test]
    fn test_sync_status_serialization() {
        let status = SyncStatus::Syncing {
            progress: 0.5,
            target_hash: "abc".to_string(),
        };

        let serialized = json::serialize_to_string(&status).unwrap();

        // Vérifie que la structure est correctement exportée pour le front-end
        assert!(serialized.contains("Syncing"));
        assert!(serialized.contains("0.5"));
        assert!(serialized.contains("abc"));
    }

    #[test]
    fn test_sync_status_error_serialization() {
        let status = SyncStatus::Error("Network timeout".into());
        let serialized = json::serialize_to_string(&status).unwrap();
        assert!(serialized.contains("Error"));
        assert!(serialized.contains("Network timeout"));
    }
}
