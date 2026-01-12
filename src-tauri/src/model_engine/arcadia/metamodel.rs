use super::common::{ElementRef, I18nString};
use serde::{Deserialize, Serialize};

/// Propriétés fonctionnelles communes (Arcadia Metamodel)
/// Tous les éléments métiers (Acteurs, Fonctions, Composants...) ont ces champs.
// CORRECTION : Ajout de `Default` ici
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ArcadiaProperties {
    #[serde(rename = "xmi_id", skip_serializing_if = "Option::is_none")]
    pub xmi_id: Option<String>,

    #[serde(default)]
    pub name: I18nString,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<I18nString>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<I18nString>,

    #[serde(default)]
    pub tags: Vec<String>,

    /// Extensions PVMT (Property Values Management Tool)
    #[serde(rename = "propertyValues", default)]
    pub property_values: Vec<ElementRef>,
}

/// Macro pour générer les structures typées en composant les socles communs.
/// Les chemins sont absolus ($crate::...) pour fonctionner depuis n'importe où.
#[macro_export]
macro_rules! arcadia_element {
    (
        $name:ident {
            $(
                $(#[$meta:meta])* // Capture les attributs (ex: #[serde(rename = "...")])
                $field:ident : $type:ty
            ),* $(,)?
        }
    ) => {
        #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
        pub struct $name {
            // Socle technique (ID, Dates...)
            #[serde(flatten)]
            pub base: $crate::model_engine::arcadia::common::BaseEntity,

            // Socle métier (Nom, Desc, Tags...)
            #[serde(flatten)]
            pub props: $crate::model_engine::arcadia::metamodel::ArcadiaProperties,

            // Champs spécifiques déclarés dans l'appel de la macro
            $(
                $(#[$meta])*
                pub $field: $type
            ),*
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_arcadia_properties_serialization() {
        let props = ArcadiaProperties {
            xmi_id: Some("xmi_1".to_string()),
            name: I18nString::String("MyElement".to_string()),
            description: None,
            summary: None,
            tags: vec!["tag1".to_string()],
            property_values: vec![],
        };

        let json = serde_json::to_string(&props).unwrap();
        assert!(json.contains("xmi_1"));
        assert!(json.contains("MyElement"));
    }
}
