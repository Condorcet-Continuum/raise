use crate::model_engine::arcadia::common::ElementRef;

// --- Configuration Item ---
arcadia_element!(ConfigurationItem {
    kind: String, // "Hardware", "Software", "SystemPart", ...

    #[serde(rename = "partNumber", skip_serializing_if = "Option::is_none")]
    part_number: Option<String>,

    #[serde(rename = "versionId", skip_serializing_if = "Option::is_none")]
    version_id: Option<String>,

    #[serde(default)]
    composition: Vec<ElementRef>,

    #[serde(rename = "allocatedPhysicalArtifacts", default)]
    allocated_physical_artifacts: Vec<ElementRef>
});

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model_engine::arcadia::common::{BaseEntity, I18nString};
    use crate::model_engine::arcadia::metamodel::ArcadiaProperties;

    #[test]
    fn test_configuration_item_ci() {
        let ci = ConfigurationItem {
            base: BaseEntity {
                id: "ci-1".into(),
                created_at: "".into(),
                modified_at: "".into(),
            },
            props: ArcadiaProperties {
                xmi_id: None,
                name: I18nString::String("FlightSoftware".into()),
                description: None,
                summary: None,
                tags: vec![],
                property_values: vec![],
            },
            kind: "Software".to_string(),
            part_number: Some("PN-12345".to_string()),
            version_id: Some("1.0.0".to_string()),
            composition: vec![],
            allocated_physical_artifacts: vec!["pc-software-comp".into()],
        };

        assert_eq!(ci.kind, "Software");
        assert_eq!(ci.part_number, Some("PN-12345".to_string()));
    }
}
