// FICHIER : src-tauri/src/utils/error.rs

use serde::Serialize;
use std::io;
use std::path::PathBuf;

// --- RE-EXPORTS ANYHOW (Strictement identique à l'original) ---
pub use anyhow::Result as AnyResult;
pub use anyhow::{anyhow, Context};

// =========================================================================
// 1. LEVÉE D'AMBIGUÏTÉ (Nouveau, mais silencieux)
// =========================================================================

/// Le nouveau type de résultat cible pour l'écosystème RAISE.
pub type RaiseResult<T> = std::result::Result<T, AppError>;

/// Type de résultat standard (L'original, conservé pour ne rien casser)
pub type Result<T> = std::result::Result<T, AppError>;

// =========================================================================
// 2. ÉNUMÉRATION DES EXCEPTIONS
// =========================================================================

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    // -----------------------------------------------------------------
    // BLOC ORIGINAL INTRAITABLE (Zéro modification de signature)
    // -----------------------------------------------------------------
    #[error("Erreur de configuration : {0}")]
    Config(String),

    #[error("Erreur d'entrée/sortie : {0}")]
    Io(#[from] io::Error),

    #[error("Erreur Base de Données : {0}")]
    Database(String),

    #[error("Erreur Réseau : {0}")]
    Network(#[from] reqwest::Error),

    #[error("Erreur IA/LLM : {0}")]
    Ai(String),

    #[error("Erreur Système : {0}")]
    System(#[from] anyhow::Error),

    #[error("Introuvable : {0}")]
    NotFound(String),

    #[error("Erreur de sérialisation : {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Validation Error: {0}")]
    Validation(String),

    // -----------------------------------------------------------------
    // BLOC NOUVEAU (Additif, utilisé uniquement par le futur code)
    // -----------------------------------------------------------------
    #[error("Échec système de fichiers [{action}] sur {path:?}: {source}")]
    FileSystem {
        action: String,
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    // 🎯 NOUVEAU : Encapsulation des erreurs Blockchain (Pattern Poupée Russe)
    #[error(transparent)]
    Blockchain(#[from] crate::blockchain::error::BlockchainError),
}

// --- SÉRIALISATION (Strictement identique à l'original) ---
impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::ser::Serializer,
    {
        serializer.serialize_str(self.to_string().as_ref())
    }
}

// --- HELPERS DE CONVERSION (Strictement identiques à l'original) ---
impl From<String> for AppError {
    fn from(s: String) -> Self {
        AppError::System(anyhow::anyhow!(s))
    }
}

impl From<&str> for AppError {
    fn from(s: &str) -> Self {
        AppError::System(anyhow::anyhow!(s.to_string()))
    }
}

// --- CONSTRUCTEURS ---
impl AppError {
    /// Crée une erreur d'Entrée/Sortie personnalisée (L'original restauré)
    pub fn custom_io(msg: impl Into<String>) -> Self {
        AppError::Io(std::io::Error::other(msg.into()))
    }

    /// Constructeur rapide pour les nouvelles erreurs de fichiers (Additif)
    pub fn fs_error(action: &str, path: impl AsRef<std::path::Path>, source: io::Error) -> Self {
        Self::FileSystem {
            action: action.to_string(),
            path: path.as_ref().to_path_buf(),
            source,
        }
    }
}

// =========================================================================
// PONTS DE CONVERSION EXTERNES (Deep Learning, etc.)
// =========================================================================

// Permet d'utiliser le `?` directement sur toutes les opérations Candle (Tensors, Models)
// Le système va automatiquement transformer l'erreur Candle en `AppError::Ai`
impl From<candle_core::Error> for AppError {
    fn from(e: candle_core::Error) -> Self {
        AppError::Ai(e.to_string())
    }
}

impl From<tera::Error> for AppError {
    fn from(e: tera::Error) -> Self {
        // On l'encapsule dans une erreur Système générique
        AppError::System(anyhow::anyhow!("Erreur de Templating Tera : {}", e))
    }
}

// --- TESTS UNITAIRES (Restaurés et complétés) ---
#[cfg(test)]
mod tests {
    use super::*;

    // Vos tests originaux intacts
    #[test]
    fn test_app_error_display_formatting() {
        let err = AppError::Config("Fichier manquant".to_string());
        assert_eq!(
            err.to_string(),
            "Erreur de configuration : Fichier manquant"
        );

        let err_db = AppError::Database("Connexion refusée".to_string());
        assert_eq!(
            err_db.to_string(),
            "Erreur Base de Données : Connexion refusée"
        );
    }

    #[test]
    fn test_app_error_serialization() {
        let err = AppError::Ai("Service indisponible".to_string());
        let json = serde_json::to_string(&err).expect("Devrait être sérialisable");
        assert_eq!(json, "\"Erreur IA/LLM : Service indisponible\"");
    }

    #[test]
    fn test_custom_io_helper() {
        let err = AppError::custom_io("Accès refusé au dossier");
        match err {
            AppError::Io(e) => {
                assert_eq!(e.kind(), std::io::ErrorKind::Other);
                assert_eq!(e.to_string(), "Accès refusé au dossier");
            }
            _ => panic!("Le helper doit générer une AppError::Io"),
        }
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

    #[test]
    fn test_from_string_helpers() {
        let err_string: AppError = String::from("Erreur string").into();
        match err_string {
            AppError::System(e) => assert_eq!(e.to_string(), "Erreur string"),
            _ => panic!("String devrait devenir AppError::System"),
        }

        let err_str: AppError = "Erreur str".into();
        match err_str {
            AppError::System(e) => assert_eq!(e.to_string(), "Erreur str"),
            _ => panic!("&str devrait devenir AppError::System"),
        }
    }

    #[test]
    fn test_from_serde_error() {
        let bad_json = "{ invalid json }";
        let serde_err = serde_json::from_str::<serde_json::Value>(bad_json).unwrap_err();
        let app_err: AppError = serde_err.into();
        match app_err {
            AppError::Serialization(e) => assert!(e.is_syntax()),
            _ => panic!("Devrait être converti en AppError::Serialization"),
        }
    }

    // Le nouveau test pour valider la fondation
    #[test]
    fn test_raise_result_compatibility() {
        fn check() -> RaiseResult<bool> {
            Ok(true)
        }
        assert!(check().unwrap());
    }
}
