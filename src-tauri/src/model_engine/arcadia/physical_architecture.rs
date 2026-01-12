use crate::arcadia_element;
use crate::model_engine::arcadia::common::ElementRef;

// --- Physical Component (Node / Behavior) ---
arcadia_element!(PhysicalComponent {
    nature: String, // "Node" | "Behavior"

    #[serde(rename = "subComponents", default)]
    sub_components: Vec<ElementRef>,

    #[serde(rename = "allocatedFunctions", default)]
    allocated_functions: Vec<ElementRef>,

    #[serde(rename = "realizedLogicalComponents", default)]
    realized_logical_components: Vec<ElementRef>,

    // Pour Behavior
    #[serde(rename = "deployedOn", default)]
    deployed_on: Vec<ElementRef>,

    // Pour Node
    #[serde(rename = "deployedComponents", default)]
    deployed_components: Vec<ElementRef>
});

// --- Physical Function ---
arcadia_element!(PhysicalFunction {
    #[serde(rename = "realizedLogicalFunctions", default)]
    realized_logical_functions: Vec<ElementRef>,

    #[serde(rename = "allocatedTo", default)]
    allocated_to: Vec<ElementRef>,

    #[serde(default)]
    inputs: Vec<ElementRef>,

    #[serde(default)]
    outputs: Vec<ElementRef>
});

// --- Physical Actor ---
arcadia_element!(PhysicalActor {
    #[serde(rename = "realizedLogicalActors", default)]
    realized_logical_actors: Vec<ElementRef>,

    #[serde(rename = "allocatedFunctions", default)]
    allocated_functions: Vec<ElementRef>
});

// --- Physical Link (Câble/Bus/Ondes) ---
arcadia_element!(PhysicalLink {
    #[serde(rename = "linkType", default)]
    link_type: String, // "Ethernet", "Bus", etc.

    source: ElementRef,
    target: ElementRef,

    #[serde(default)]
    transports: Vec<ElementRef> // ComponentExchanges
});

// --- Component Exchange (Physique) ---
arcadia_element!(PhysicalComponentExchange {
    source: ElementRef,
    target: ElementRef,

    #[serde(rename = "allocatedToPhysicalLink", default)]
    allocated_to_physical_link: Vec<ElementRef>,

    #[serde(rename = "allocatesFunctionalExchanges", default)]
    allocates_functional_exchanges: Vec<ElementRef>
});

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model_engine::arcadia::common::{BaseEntity, I18nString};
    use crate::model_engine::arcadia::metamodel::ArcadiaProperties;

    #[test]
    fn test_physical_component_node() {
        let node = PhysicalComponent {
            base: BaseEntity {
                id: "pc-1".into(),
                created_at: "".into(),
                modified_at: "".into(),
            },
            props: ArcadiaProperties {
                xmi_id: None,
                name: I18nString::String("Server".into()),
                description: None,
                summary: None,
                tags: vec![],
                property_values: vec![],
            },
            nature: "Node".to_string(),
            sub_components: vec![],
            allocated_functions: vec![],
            realized_logical_components: vec![],
            deployed_on: vec![],
            deployed_components: vec!["pc-2-behavior".into()], // Héberge un composant comportemental
        };

        assert_eq!(node.nature, "Node");
        assert_eq!(node.deployed_components.len(), 1);
    }
}
