// src-tauri/src/blockchain/sync/state.rs

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum SyncStatus {
    /// Le nœud vient de démarrer et cherche des pairs.
    Initializing,
    /// Le nœud est en train de télécharger des commits manquants.
    Syncing { progress: f32, target_hash: String },
    /// Le nœud est à jour avec le quorum détecté.
    UpToDate,
    /// Erreur de synchronisation (ex: fork non résolu).
    Error(String),
}

/// Implémentation par défaut pour faciliter l'initialisation des structures parentes.
impl Default for SyncStatus {
    fn default() -> Self {
        Self::Initializing
    }
}

// --- TESTS UNITAIRES ---

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

        let serialized = serde_json::to_string(&status).unwrap();

        // Vérifie que la structure est correctement exportée pour le front-end
        assert!(serialized.contains("Syncing"));
        assert!(serialized.contains("0.5"));
        assert!(serialized.contains("abc"));
    }

    #[test]
    fn test_sync_status_error_serialization() {
        let status = SyncStatus::Error("Network timeout".into());
        let serialized = serde_json::to_string(&status).unwrap();
        assert!(serialized.contains("Error"));
        assert!(serialized.contains("Network timeout"));
    }
}
