// FICHIER : src-tauri/src/utils/core/error.rs

// 1. Data : Typage JSON et Sérialisation
use crate::utils::data::json::{json_value, JsonValue};
use crate::utils::data::Serializable;

// --- RE-EXPORTS ANYHOW ---
// On ré-exporte anyhow pour que les services puissent l'utiliser sans l'importer explicitement.
pub use anyhow::Result as AnyResult;
pub use anyhow::{anyhow, Context};

// =========================================================================
// 1. LEVÉE D'AMBIGUÏTÉ (Le standard RAISE)
// =========================================================================

/// 🤖 IA NOTE: Vous devez TOUJOURS retourner RaiseResult<T> dans vos fonctions métier.
pub type RaiseResult<T> = std::result::Result<T, AppError>;

// =========================================================================
// 2. ÉNUMÉRATION DES EXCEPTIONS (Observabilité IA)
// =========================================================================

#[derive(Debug)]
pub struct StructuredData {
    pub service: String,
    pub subdomain: String,
    pub component: String,
    pub code: String,
    pub message: String,
    pub context: JsonValue, // 🎯 Remplacé par notre type sémantique AI-Ready
}

impl std::fmt::Display for StructuredData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "[{:?}::{:?}::{:?}] {}: {}",
            self.service, self.subdomain, self.component, self.code, self.message
        )
    }
}

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    /// 🎯 VARIANT FLAGSHIP : Pour l'observabilité totale par l'IA et le monitoring.
    /// Il capture l'origine précise (service/composant) et le contexte JSON.
    #[error("{0}")]
    Structured(Box<StructuredData>),
}

// =========================================================================
// CONSTRUCTEURS OFFICIELS (La Façade)
// =========================================================================

impl AppError {
    /// 🎯 Constructeur pour une erreur métier intentionnellement silencieuse.
    /// Ne déclenche aucun log automatique (idéal pour les vérifications de routine type 'NotFound').
    pub fn silent_not_found(
        service: &str,
        subdomain: &str,
        component: &str,
        message: &str,
        context: JsonValue,
    ) -> Self {
        AppError::Structured(Box::new(StructuredData {
            service: service.to_string(),
            subdomain: subdomain.to_string(),
            component: component.to_string(),
            code: "ERR_NOT_FOUND_SILENT".to_string(),
            message: message.to_string(),
            context,
        }))
    }
}

// =========================================================================
// 3. CONTRAT DE DONNÉES AVEC LE FRONTEND
// =========================================================================

impl Serializable for AppError {
    // 🎯 Utilisation de notre trait sémantique
    /// Sérialise l'erreur pour le Frontend Tauri.
    /// IMPORTANT : Pour le variant `Structured`, on n'envoie que le `message` traduit.
    /// Le contexte technique (ID, logs) reste côté Backend pour la sécurité et l'IA.
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer, // Tuyauterie interne maintenue avec serde
    {
        // On destructure directement l'unique variant de l'enum
        let AppError::Structured(data) = self;
        serializer.serialize_str(data.message.as_str())
    }
}

// =========================================================================
// 4. CONSTRUCTEURS SÉMANTIQUES (En cours de dépréciation)
// =========================================================================

// --- CONVERSIONS DE TYPES VIA MACRO INTERNE ---
// Ces implémentations utilisent déjà build_error! sous le capot
// et génèrent donc le variant Structured de manière propre.

impl From<candle_core::Error> for AppError {
    fn from(e: candle_core::Error) -> Self {
        crate::build_error!(
            "ERR_AI_MODEL_EXECUTION",
            error = format!("Erreur interne du modèle IA : {}", e),
            context = json_value!({"engine": "candle_core"})
        )
    }
}

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        crate::build_error!(
            "ERR_SYSTEM_IO",
            error = format!("Erreur d'accès fichier ou réseau (IO) : {}", e),
            context = json_value!({
                "os_error": e.to_string(),
                "error_kind": format!("{:?}", e.kind())
            })
        )
    }
}

// =========================================================================
// 5. TESTS UNITAIRES
// =========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::raise_error;
    use crate::utils::data::json;
    use crate::utils::data::json::json_value;

    #[test]
    fn test_structured_error_frontend_serialization() {
        // ✅ Utilisation de la Box et de StructuredData
        let err = AppError::Structured(Box::new(StructuredData {
            service: "ai".to_string(),
            subdomain: "nlp".to_string(),
            component: "PARSER".to_string(),
            code: "ERR_TEST_01".to_string(),
            message: "Message lisible par l'utilisateur".to_string(),
            context: json_value!({ "action": "TEST_ACTION" }), // 🎯 Remplacé
        }));

        // 🎯 Utilisation de notre fonction de façade "serialize_to_string"
        let serialized = json::serialize_to_string(&err).expect("Doit être sérialisable");
        // Le Frontend ne doit recevoir QUE le message texte
        assert_eq!(serialized, "\"Message lisible par l'utilisateur\"");
    }

    #[test]
    fn test_legacy_fs_error_behavior() {
        fn trigger_legacy_error() -> RaiseResult<()> {
            let path = "test.txt";
            let action = "READ";

            // Note : raise_error utilise i18n::t pour le champ 'message'.
            // L'argument 'error =' va maintenant dans 'context.technical_error'
            raise_error!(
                "ERR_FS_READ",
                error = format!("Erreur lors de l'action {} sur {}", action, path),
                context = json_value!({ // 🎯 Remplacé
                    "path": path,
                    "action": action
                })
            );

            #[allow(unreachable_code)]
            Ok(())
        }

        let result = trigger_legacy_error();
        assert!(result.is_err());
        let AppError::Structured(data) = result.unwrap_err();

        assert_eq!(data.code, "ERR_FS_READ");
        // On vérifie que le chemin est présent dans le CONTEXTE technique
        assert_eq!(data.context["path"], "test.txt");
        assert_eq!(
            data.context["technical_error"]
                .as_str()
                .unwrap()
                .contains("test.txt"),
            true
        );
    }

    #[test]
    fn test_explicit_anyhow_capture() {
        use anyhow::anyhow;

        fn trigger_error() -> RaiseResult<()> {
            fn external_task() -> anyhow::Result<()> {
                Err(anyhow!("Test crash"))
            }

            match external_task() {
                Ok(_) => Ok(()),
                Err(e) => {
                    raise_error!(
                        "ERR_EXTERNAL_SYSTEM",
                        error = e.to_string(),
                        context = json_value!({ "source": "anyhow_task" }) // 🎯 Remplacé
                    );
                    #[allow(unreachable_code)]
                    Ok(())
                }
            }
        }

        let result = trigger_error();
        assert!(result.is_err());
        let AppError::Structured(data) = result.unwrap_err();

        assert_eq!(data.code, "ERR_EXTERNAL_SYSTEM");
        assert_eq!(data.context["technical_error"], "Test crash");
    }
}
