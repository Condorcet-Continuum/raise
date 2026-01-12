use crate::model_engine::arcadia::common::ElementRef;

// --- Logical Component ---
arcadia_element!(LogicalComponent {
    #[serde(rename = "isAbstract", default)]
    is_abstract: bool,

    #[serde(rename = "subComponents", default)]
    sub_components: Vec<ElementRef>,

    #[serde(rename = "allocatedFunctions", default)]
    allocated_functions: Vec<ElementRef>,

    #[serde(rename = "realizedSystemComponents", default)]
    realized_system_components: Vec<ElementRef>,

    #[serde(rename = "providedInterfaces", default)]
    provided_interfaces: Vec<ElementRef>,

    #[serde(rename = "requiredInterfaces", default)]
    required_interfaces: Vec<ElementRef>
});

// --- Logical Function ---
arcadia_element!(LogicalFunction {
    #[serde(rename = "realizedSystemFunctions", default)]
    realized_system_functions: Vec<ElementRef>,

    #[serde(rename = "allocatedTo", default)]
    allocated_to: Vec<ElementRef>,

    #[serde(default)]
    inputs: Vec<ElementRef>,

    #[serde(default)]
    outputs: Vec<ElementRef>,

    #[serde(rename = "subFunctions", default)]
    sub_functions: Vec<ElementRef>
});

// --- Logical Actor ---
arcadia_element!(LogicalActor {
    #[serde(rename = "isHuman", default)]
    is_human: bool,

    #[serde(rename = "realizedSystemActors", default)]
    realized_system_actors: Vec<ElementRef>,

    #[serde(rename = "allocatedFunctions", default)]
    allocated_functions: Vec<ElementRef>
});

// --- Functional Exchange (LA) ---
arcadia_element!(LogicalFunctionalExchange {
    source: ElementRef,
    target: ElementRef,

    #[serde(rename = "exchangeItems", default)]
    exchange_items: Vec<ElementRef>,

    #[serde(rename = "realizedSystemExchanges", default)]
    realized_system_exchanges: Vec<ElementRef>,

    #[serde(rename = "allocatedToComponentExchange", default)]
    allocated_to_component_exchange: Vec<ElementRef>
});

// --- Component Exchange (Logique) ---
arcadia_element!(LogicalComponentExchange {
    source: ElementRef,
    target: ElementRef,

    #[serde(rename = "allocatesFunctionalExchanges", default)]
    allocates_functional_exchanges: Vec<ElementRef>,

    #[serde(default)]
    orientation: String // "Unidirectional" | "Bidirectional"
});

// --- Logical Interface ---
arcadia_element!(LogicalInterface {
    #[serde(rename = "isProvidedBy", default)]
    is_provided_by: Vec<ElementRef>,

    #[serde(rename = "isRequiredBy", default)]
    is_required_by: Vec<ElementRef>,

    #[serde(rename = "exchangeItems", default)]
    exchange_items: Vec<ElementRef>
});

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model_engine::arcadia::common::{BaseEntity, I18nString};
    use crate::model_engine::arcadia::metamodel::ArcadiaProperties;

    #[test]
    fn test_logical_component_structure() {
        let comp = LogicalComponent {
            base: BaseEntity {
                id: "lc-1".into(),
                created_at: "".into(),
                modified_at: "".into(),
            },
            props: ArcadiaProperties {
                xmi_id: None,
                name: I18nString::String("Controller".into()),
                description: None,
                summary: None,
                tags: vec![],
                property_values: vec![],
            },
            is_abstract: false,
            sub_components: vec![],
            allocated_functions: vec!["lf-1".into()],
            realized_system_components: vec![],
            provided_interfaces: vec![],
            required_interfaces: vec![],
        };

        assert_eq!(comp.props.name, I18nString::String("Controller".into()));
        assert!(comp.allocated_functions.contains(&"lf-1".to_string()));
    }
}
