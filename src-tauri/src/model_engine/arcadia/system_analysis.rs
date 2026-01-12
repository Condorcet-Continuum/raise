use crate::arcadia_element;
use crate::model_engine::arcadia::common::ElementRef;

// --- System Component (Le Système) ---
arcadia_element!(SystemComponent {
    #[serde(rename = "realizedEntities", default)]
    realized_entities: Vec<ElementRef>,

    #[serde(rename = "allocatedFunctions", default)]
    allocated_functions: Vec<ElementRef>
});

// --- System Actor ---
arcadia_element!(SystemActor {
    #[serde(rename = "isHuman", default)]
    is_human: bool,

    #[serde(rename = "realizedActors", default)]
    realized_actors: Vec<ElementRef>,

    #[serde(rename = "allocatedFunctions", default)]
    allocated_functions: Vec<ElementRef>
});

// --- System Function ---
arcadia_element!(SystemFunction {
    #[serde(rename = "realizedActivities", default)]
    realized_activities: Vec<ElementRef>,

    #[serde(rename = "allocatedTo", default)]
    allocated_to: Vec<ElementRef>,

    #[serde(default)]
    inputs: Vec<ElementRef>,

    #[serde(default)]
    outputs: Vec<ElementRef>
});

// --- System Capability ---
arcadia_element!(SystemCapability {
    #[serde(rename = "realizedCapabilities", default)]
    realized_capabilities: Vec<ElementRef>,

    #[serde(rename = "involvedFunctions", default)]
    involved_functions: Vec<ElementRef>,

    #[serde(rename = "involvedChains", default)]
    involved_chains: Vec<ElementRef>,

    #[serde(default)]
    scenarios: Vec<ElementRef>
});

// --- Functional Exchange (SA) ---
arcadia_element!(SystemFunctionalExchange {
    source: ElementRef,
    target: ElementRef,

    #[serde(rename = "exchangeItems", default)]
    exchange_items: Vec<ElementRef>,

    #[serde(rename = "realizedExchanges", default)]
    realized_exchanges: Vec<ElementRef>
});

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model_engine::arcadia::common::{BaseEntity, I18nString};
    use crate::model_engine::arcadia::metamodel::ArcadiaProperties;

    #[test]
    fn test_system_function_instantiation() {
        let func = SystemFunction {
            base: BaseEntity {
                id: "sf-1".to_string(),
                created_at: String::new(),
                modified_at: String::new(),
            },
            props: ArcadiaProperties {
                xmi_id: None,
                name: I18nString::String("Compute".to_string()),
                description: None,
                summary: None,
                tags: vec![],
                property_values: vec![],
            },
            realized_activities: vec![],
            allocated_to: vec!["sys-1".to_string()],
            inputs: vec![],
            outputs: vec![],
        };

        assert_eq!(func.allocated_to.len(), 1);
        assert_eq!(func.props.name, I18nString::String("Compute".to_string()));
    }
}
