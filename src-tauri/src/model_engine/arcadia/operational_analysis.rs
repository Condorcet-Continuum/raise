use crate::model_engine::arcadia::common::{ElementRef, I18nString};

// --- Operational Actor ---
arcadia_element!(OperationalActor {
    #[serde(rename = "isHuman", default)]
    is_human: bool,

    #[serde(skip_serializing_if = "Option::is_none")]
    organization: Option<ElementRef>,

    #[serde(rename = "allocatedActivities", default)]
    allocated_activities: Vec<ElementRef>
});

// --- Operational Entity ---
arcadia_element!(OperationalEntity {
    #[serde(default)]
    composition: Vec<ElementRef>,

    #[serde(rename = "allocatedActivities", default)]
    allocated_activities: Vec<ElementRef>
});

// --- Operational Activity ---
arcadia_element!(OperationalActivity {
    #[serde(default)]
    inputs: Vec<ElementRef>,

    #[serde(default)]
    outputs: Vec<ElementRef>,

    #[serde(rename = "allocatedTo", default)]
    allocated_to: Vec<ElementRef>
});

// --- Operational Capability ---
arcadia_element!(OperationalCapability {
    #[serde(rename = "involvedActivities", default)]
    involved_activities: Vec<ElementRef>,

    #[serde(rename = "involvedActors", default)]
    involved_actors: Vec<ElementRef>,

    #[serde(default)]
    scenarios: Vec<ElementRef>
});

// --- Operational Exchange ---
arcadia_element!(OperationalExchange {
    source: ElementRef,
    target: ElementRef,

    #[serde(rename = "exchangeItems", default)]
    exchange_items: Vec<I18nString>,

    #[serde(rename = "flowType", default)]
    flow_type: String
});

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model_engine::arcadia::common::BaseEntity;
    use crate::model_engine::arcadia::metamodel::ArcadiaProperties;

    #[test]
    fn test_operational_actor_macro_expansion() {
        // On vérifie que la structure générée par la macro fonctionne
        let actor = OperationalActor {
            base: BaseEntity {
                id: "oa-1".to_string(),
                created_at: "".to_string(),
                modified_at: "".to_string(),
            },
            props: ArcadiaProperties {
                xmi_id: None,
                name: I18nString::String("User".to_string()),
                description: None,
                summary: None,
                tags: vec![],
                property_values: vec![],
            },
            is_human: true,
            organization: None,
            allocated_activities: vec!["act-1".to_string()],
        };

        let json_val = serde_json::to_value(&actor).unwrap();

        // Vérification du Flattening
        assert_eq!(json_val["id"], "oa-1"); // BaseEntity
        assert_eq!(json_val["name"], "User"); // ArcadiaProperties
        assert_eq!(json_val["isHuman"], true); // OperationalActor specific
    }
}
