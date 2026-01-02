use serde::Serialize;
use std::io;

/// Type de résultat standard pour l'application RAISE
pub type Result<T> = std::result::Result<T, AppError>;

/// Enumération centrale des erreurs de l'application.
/// Elle dérive `thiserror::Error` pour faciliter la conversion automatique.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("Erreur de configuration : {0}")]
    Config(String),

    #[error("Erreur d'entrée/sortie : {0}")]
    Io(#[from] io::Error),

    #[error("Erreur Base de Données : {0}")]
    Database(String),

    #[error("Erreur IA/LLM : {0}")]
    Ai(String),

    #[error("Erreur Système : {0}")]
    System(#[from] anyhow::Error),

    #[error("Introuvable : {0}")]
    NotFound(String),

    #[error("Erreur de sérialisation : {0}")]
    Serialization(#[from] serde_json::Error),
}

// Implémentation manuelle de Serialize pour renvoyer l'erreur au Frontend
// via les Commandes Tauri (Tauri exige que les erreurs soient sérialisables).
impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::ser::Serializer,
    {
        serializer.serialize_str(self.to_string().as_ref())
    }
}

// Helpers pour convertir des erreurs string en AppError
impl From<String> for AppError {
    fn from(s: String) -> Self {
        AppError::System(anyhow::anyhow!(s))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_error_display_formatting() {
        let err = AppError::Config("Fichier manquant".to_string());
        assert_eq!(
            err.to_string(),
            "Erreur de configuration : Fichier manquant"
        );

        let err_not_found = AppError::NotFound("Utilisateur".to_string());
        assert_eq!(err_not_found.to_string(), "Introuvable : Utilisateur");
    }

    #[test]
    fn test_app_error_serialization() {
        // Test critique : Tauri a besoin que l'erreur soit sérialisée en String simple
        // pour que le frontend puisse l'afficher directement.
        let err = AppError::Ai("Service indisponible".to_string());
        let json = serde_json::to_string(&err).expect("Devrait être sérialisable");

        // Notre implémentation personnalisée de Serialize doit renvoyer juste la chaîne
        assert_eq!(json, "\"Erreur IA/LLM : Service indisponible\"");
    }

    #[test]
    fn test_from_io_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::TimedOut, "Timeout réseau");
        let app_err: AppError = io_err.into();

        match app_err {
            AppError::Io(msg) => assert!(msg.to_string().contains("Timeout réseau")),
            _ => panic!("Devrait être converti en AppError::Io"),
        }
    }

    #[test]
    fn test_from_anyhow_error() {
        let anyhow_err = anyhow::anyhow!("Erreur inconnue");
        let app_err: AppError = anyhow_err.into();

        match app_err {
            AppError::System(err) => assert_eq!(err.to_string(), "Erreur inconnue"),
            _ => panic!("Devrait être converti en AppError::System"),
        }
    }
}
