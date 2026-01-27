// FICHIER : src-tauri/src/model_engine/arcadia/mod.rs

/// Ce module contient le référentiel sémantique d'Arcadia.
/// Il définit les URIs officielles et les noms de propriétés utilisés dans le JSON-DB.
/// Il remplace les anciennes structures rigides par une approche orientée "Données".
// On conserve element_kind s'il existe (pour la logique métier is_structural/is_behavioral)
// Si vous n'avez pas ce fichier, vous pourrez le créer ou supprimer cette ligne.
pub mod element_kind;

// --- 1. OPERATIONAL ANALYSIS (OA) ---
pub const KIND_OA_ACTOR: &str = "https://raise.io/ontology/arcadia/oa#OperationalActor";
pub const KIND_OA_ACTIVITY: &str = "https://raise.io/ontology/arcadia/oa#OperationalActivity";
pub const KIND_OA_CAPABILITY: &str = "https://raise.io/ontology/arcadia/oa#OperationalCapability";
pub const KIND_OA_EXCHANGE: &str = "https://raise.io/ontology/arcadia/oa#OperationalExchange";
pub const KIND_OA_ENTITY: &str = "https://raise.io/ontology/arcadia/oa#OperationalEntity";

// --- 2. SYSTEM ANALYSIS (SA) ---
pub const KIND_SA_FUNCTION: &str = "https://raise.io/ontology/arcadia/sa#SystemFunction";
pub const KIND_SA_COMPONENT: &str = "https://raise.io/ontology/arcadia/sa#SystemComponent";
pub const KIND_SA_ACTOR: &str = "https://raise.io/ontology/arcadia/sa#SystemActor";
pub const KIND_SA_EXCHANGE: &str = "https://raise.io/ontology/arcadia/sa#SystemFunctionalExchange";
pub const KIND_SA_CAPABILITY: &str = "https://raise.io/ontology/arcadia/sa#SystemCapability";

// --- 3. LOGICAL ARCHITECTURE (LA) ---
pub const KIND_LA_FUNCTION: &str = "https://raise.io/ontology/arcadia/la#LogicalFunction";
pub const KIND_LA_COMPONENT: &str = "https://raise.io/ontology/arcadia/la#LogicalComponent";
pub const KIND_LA_ACTOR: &str = "https://raise.io/ontology/arcadia/la#LogicalActor";
pub const KIND_LA_INTERFACE: &str = "https://raise.io/ontology/arcadia/la#LogicalInterface";
pub const KIND_LA_EXCHANGE: &str = "https://raise.io/ontology/arcadia/la#FunctionalExchange";

// --- 4. PHYSICAL ARCHITECTURE (PA) ---
pub const KIND_PA_FUNCTION: &str = "https://raise.io/ontology/arcadia/pa#PhysicalFunction";
pub const KIND_PA_COMPONENT: &str = "https://raise.io/ontology/arcadia/pa#PhysicalComponent";
pub const KIND_PA_LINK: &str = "https://raise.io/ontology/arcadia/pa#PhysicalLink";
pub const KIND_PA_ACTOR: &str = "https://raise.io/ontology/arcadia/pa#PhysicalActor"; // Souvent un SystemActor alloué

// --- 5. DATA & COMMON ---
pub const KIND_DATA_CLASS: &str = "https://raise.io/ontology/arcadia/data#Class";
pub const KIND_UNKNOWN: &str = "Unknown";

// --- 6. CLÉS DE PROPRIÉTÉS JSON (Vocabulaire Technique) ---
// Utiliser ces constantes évite les fautes de frappe dans le code (ex: "alocatedFunctions")
pub const PROP_NAME: &str = "name";
pub const PROP_ID: &str = "id";
pub const PROP_DESCRIPTION: &str = "description";
pub const PROP_ALLOCATED_FUNCTIONS: &str = "allocatedFunctions";
pub const PROP_OWNED_LOGICAL_COMPONENTS: &str = "ownedLogicalComponents";
pub const PROP_OWNED_SYSTEM_COMPONENTS: &str = "ownedSystemComponents";
pub const PROP_INCOMING_EXCHANGES: &str = "incomingFunctionalExchanges";
pub const PROP_OUTGOING_EXCHANGES: &str = "outgoingFunctionalExchanges";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_uris_are_well_formed() {
        // Vérification basique de format
        assert!(KIND_OA_ACTOR.starts_with("https://raise.io/ontology/arcadia/oa#"));
        assert!(KIND_SA_COMPONENT.starts_with("https://raise.io/ontology/arcadia/sa#"));
        assert!(KIND_LA_FUNCTION.ends_with("LogicalFunction"));
    }

    #[test]
    fn test_property_keys_correctness() {
        // Ces clés sont critiques pour le mapping JSON, on vérifie qu'elles ne changent pas par erreur
        assert_eq!(PROP_NAME, "name");
        assert_eq!(PROP_ALLOCATED_FUNCTIONS, "allocatedFunctions");
        assert_eq!(PROP_INCOMING_EXCHANGES, "incomingFunctionalExchanges");
    }
}
