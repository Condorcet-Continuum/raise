use crate::model_engine::arcadia::common::ElementRef;

// --- Class (UML-like) ---
arcadia_element!(Class {
    #[serde(rename = "isAbstract", default)]
    is_abstract: bool,

    #[serde(rename = "superClasses", default)]
    super_classes: Vec<ElementRef>,

    #[serde(rename = "ownedFeatures", default)]
    properties: Vec<ElementRef> // Références vers des "Property"
});

// --- Exchange Item (Ce qui circule dans un flux) ---
arcadia_element!(ExchangeItem {
    #[serde(rename = "exchangeMechanism", default)]
    exchange_mechanism: String, // "Unset", "Flow", "Operation", "Event"

    #[serde(rename = "ownedElements", default)]
    elements: Vec<ElementRef> // Références vers des ExchangeItemElement
});

// --- Data Type (Boolean, Enumeration, Numeric) ---
arcadia_element!(DataType {
    #[serde(default)]
    kind: String, // "BooleanType", "Enumeration", "NumericType", "StringType"

    #[serde(default)]
    pattern: Option<String>, // Regex pour StringType

    #[serde(rename = "ownedLiterals", default)]
    literals: Vec<ElementRef> // Pour Enumeration
});

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model_engine::arcadia::common::{BaseEntity, I18nString};
    use crate::model_engine::arcadia::metamodel::ArcadiaProperties;

    #[test]
    fn test_class_structure() {
        let cls = Class {
            base: BaseEntity {
                id: "cls-1".into(),
                created_at: "".into(),
                modified_at: "".into(),
            },
            props: ArcadiaProperties {
                name: I18nString::String("Telemetry".into()),
                description: None,
                ..Default::default()
            },
            is_abstract: false,
            super_classes: vec![],
            properties: vec!["prop-speed".into(), "prop-alt".into()],
        };

        assert_eq!(cls.props.name, I18nString::String("Telemetry".into()));
        assert_eq!(cls.properties.len(), 2);
    }
}
