// FICHIER : src-tauri/src/model_engine/arcadia/mod.rs

use crate::json_db::jsonld::VocabularyRegistry;

/// Ce module contient le référentiel sémantique d'Arcadia.
/// Il fait le pont entre le code statique (constantes) et le registre dynamique (JSON-LD).
pub mod element_kind;

// --- 1. CONSTANTES LEGACY (Pour compatibilité immédiate) ---
// Ces constantes sont validées par test contre le VocabularyRegistry.
// À terme, le loader utilisera directement le registre.

// OPERATIONAL ANALYSIS (OA)
pub const KIND_OA_ACTOR: &str = "https://raise.io/ontology/arcadia/oa#OperationalActor";
pub const KIND_OA_ACTIVITY: &str = "https://raise.io/ontology/arcadia/oa#OperationalActivity";
pub const KIND_OA_CAPABILITY: &str = "https://raise.io/ontology/arcadia/oa#OperationalCapability";
pub const KIND_OA_EXCHANGE: &str = "https://raise.io/ontology/arcadia/oa#OperationalExchange";
pub const KIND_OA_ENTITY: &str = "https://raise.io/ontology/arcadia/oa#OperationalEntity";

// SYSTEM ANALYSIS (SA)
pub const KIND_SA_FUNCTION: &str = "https://raise.io/ontology/arcadia/sa#SystemFunction";
pub const KIND_SA_COMPONENT: &str = "https://raise.io/ontology/arcadia/sa#SystemComponent";
pub const KIND_SA_ACTOR: &str = "https://raise.io/ontology/arcadia/sa#SystemActor";
pub const KIND_SA_EXCHANGE: &str = "https://raise.io/ontology/arcadia/sa#SystemFunctionalExchange"; // Note: Mappé vers FunctionalExchange
pub const KIND_SA_CAPABILITY: &str = "https://raise.io/ontology/arcadia/sa#SystemCapability";

// LOGICAL ARCHITECTURE (LA)
pub const KIND_LA_FUNCTION: &str = "https://raise.io/ontology/arcadia/la#LogicalFunction";
pub const KIND_LA_COMPONENT: &str = "https://raise.io/ontology/arcadia/la#LogicalComponent";
pub const KIND_LA_ACTOR: &str = "https://raise.io/ontology/arcadia/la#LogicalActor";
pub const KIND_LA_INTERFACE: &str = "https://raise.io/ontology/arcadia/la#LogicalInterface";
pub const KIND_LA_EXCHANGE: &str = "https://raise.io/ontology/arcadia/la#FunctionalExchange";

// PHYSICAL ARCHITECTURE (PA)
pub const KIND_PA_FUNCTION: &str = "https://raise.io/ontology/arcadia/pa#PhysicalFunction";
pub const KIND_PA_COMPONENT: &str = "https://raise.io/ontology/arcadia/pa#PhysicalComponent";
pub const KIND_PA_LINK: &str = "https://raise.io/ontology/arcadia/pa#PhysicalLink";
pub const KIND_PA_ACTOR: &str = "https://raise.io/ontology/arcadia/pa#PhysicalActor";

// EPBS
pub const KIND_EPBS_ITEM: &str = "https://raise.io/ontology/arcadia/epbs#ConfigurationItem";

// DATA & COMMON
pub const KIND_DATA_CLASS: &str = "https://raise.io/ontology/arcadia/data#Class";
pub const KIND_UNKNOWN: &str = "Unknown";

// --- 2. CLÉS DE PROPRIÉTÉS JSON (Vocabulaire Technique) ---
pub const PROP_NAME: &str = "name";
pub const PROP_ID: &str = "id";
pub const PROP_DESCRIPTION: &str = "description";
pub const PROP_ALLOCATED_FUNCTIONS: &str = "allocatedFunctions";
pub const PROP_OWNED_LOGICAL_COMPONENTS: &str = "ownedLogicalComponents";
pub const PROP_OWNED_SYSTEM_COMPONENTS: &str = "ownedSystemComponents";
pub const PROP_INCOMING_EXCHANGES: &str = "incomingFunctionalExchanges";
pub const PROP_OUTGOING_EXCHANGES: &str = "outgoingFunctionalExchanges";

// --- 3. ACCÈS DYNAMIQUE (MBSE 2.0) ---

pub struct ArcadiaOntology;

impl ArcadiaOntology {
    /// Récupère l'URI complète d'un type via le registre central.
    /// Exemple: ("oa", "OperationalActor") -> Some("https://.../oa#OperationalActor")
    pub fn get_uri(layer_prefix: &str, type_name: &str) -> Option<String> {
        let reg = VocabularyRegistry::global();
        let default_ctx = reg.get_default_context();

        // Correction Clippy : Utilisation de .map() au lieu de if let Some
        default_ctx
            .get(layer_prefix)
            .map(|ns| format!("{}{}", ns, type_name))
    }

    /// Vérifie si une URI donnée correspond bien à un type connu dans le registre
    pub fn is_known_type(uri: &str) -> bool {
        VocabularyRegistry::global().has_class(uri)
    }
}

// ============================================================================
// TESTS DE VALIDATION CROISÉE
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json_db::jsonld::vocabulary::{arcadia_types, namespaces};

    #[test]
    fn test_constants_match_registry_definitions() {
        // Ce test garantit que les constantes "hardcoded" ici sont strictement identiques
        // à ce qui est défini dans vocabulary.rs. C'est le filet de sécurité.

        // OA
        let expected_oa_actor = format!("{}{}", namespaces::OA, arcadia_types::OA_ACTOR);
        assert_eq!(
            KIND_OA_ACTOR, expected_oa_actor,
            "Désynchronisation OA_ACTOR !"
        );

        // SA
        let expected_sa_func = format!("{}{}", namespaces::SA, arcadia_types::SA_FUNCTION);
        assert_eq!(
            KIND_SA_FUNCTION, expected_sa_func,
            "Désynchronisation SA_FUNCTION !"
        );

        // PA
        let expected_pa_comp = format!("{}{}", namespaces::PA, arcadia_types::PA_COMPONENT);
        assert_eq!(
            KIND_PA_COMPONENT, expected_pa_comp,
            "Désynchronisation PA_COMPONENT !"
        );
    }

    #[test]
    fn test_dynamic_lookup() {
        let uri = ArcadiaOntology::get_uri("oa", "OperationalActivity").unwrap();
        assert!(uri.contains("raise.io/ontology/arcadia/oa#"));
        assert!(uri.ends_with("OperationalActivity"));
    }

    #[test]
    fn test_property_keys_integrity() {
        // Ces clés sont critiques pour le mapping JSON.
        assert_eq!(PROP_NAME, "name");
        assert_eq!(PROP_ID, "id");
        assert_eq!(PROP_ALLOCATED_FUNCTIONS, "allocatedFunctions");
    }
}
