// FICHIER : src-tauri/src/json_db/jsonld/vocabulary.rs

use crate::utils::{
    error::AnyResult,
    fs::Path,
    json::{self, Deserialize, Serialize, Value},
    Arc, HashMap, OnceLock, RwLock,
};
// --- NAMESPACES ---
pub mod namespaces {
    pub const ARCADIA: &str = "https://raise.io/ontology/arcadia#";
    pub const OA: &str = "https://raise.io/ontology/arcadia/oa#";
    pub const SA: &str = "https://raise.io/ontology/arcadia/sa#";
    pub const LA: &str = "https://raise.io/ontology/arcadia/la#";
    pub const PA: &str = "https://raise.io/ontology/arcadia/pa#";
    pub const EPBS: &str = "https://raise.io/ontology/arcadia/epbs#";
    pub const DATA: &str = "https://raise.io/ontology/arcadia/data#";
    pub const TRANSVERSE: &str = "https://raise.io/ontology/arcadia/transverse#";

    // Standards
    pub const RDF: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#";
    pub const RDFS: &str = "http://www.w3.org/2000/01/rdf-schema#";
    pub const OWL: &str = "http://www.w3.org/2002/07/owl#";
    pub const XSD: &str = "http://www.w3.org/2001/XMLSchema#";
    pub const DCTERMS: &str = "http://purl.org/dc/terms/";
    pub const PROV: &str = "http://www.w3.org/ns/prov#";
}

// --- CONSTANTES DE TYPAGE ---
pub mod arcadia_types {
    // OA
    pub const OA_ACTOR: &str = "OperationalActor";
    pub const OA_ACTIVITY: &str = "OperationalActivity";
    pub const OA_CAPABILITY: &str = "OperationalCapability";
    pub const OA_ENTITY: &str = "OperationalEntity";
    pub const OA_EXCHANGE: &str = "OperationalExchange";

    // SA
    pub const SA_COMPONENT: &str = "SystemComponent";
    pub const SA_FUNCTION: &str = "SystemFunction";
    pub const SA_ACTOR: &str = "SystemActor";
    pub const SA_CAPABILITY: &str = "SystemCapability";
    pub const SA_EXCHANGE: &str = "FunctionalExchange";

    // LA
    pub const LA_COMPONENT: &str = "LogicalComponent";
    pub const LA_FUNCTION: &str = "LogicalFunction";
    pub const LA_ACTOR: &str = "LogicalActor";
    pub const LA_INTERFACE: &str = "LogicalInterface";

    // PA
    pub const PA_COMPONENT: &str = "PhysicalComponent";
    pub const PA_FUNCTION: &str = "PhysicalFunction";
    pub const PA_ACTOR: &str = "PhysicalActor";
    pub const PA_LINK: &str = "PhysicalLink";

    // EPBS
    pub const EPBS_ITEM: &str = "ConfigurationItem";

    // DATA
    pub const DATA_CLASS: &str = "Class";
    pub const DATA_TYPE: &str = "DataType";
    pub const EXCHANGE_ITEM: &str = "ExchangeItem";

    // TRANSVERSE (Mise à jour suite à la structure réelle)
    pub const TRANSVERSE_REQUIREMENT: &str = "Requirement";
    pub const TRANSVERSE_SCENARIO: &str = "Scenario";
    pub const TRANSVERSE_FUNCTIONAL_CHAIN: &str = "FunctionalChain";
    pub const TRANSVERSE_CONSTRAINT: &str = "Constraint";
    pub const TRANSVERSE_QUALITY_RULE: &str = "QualityRule";
    pub const TRANSVERSE_TEST_PROCEDURE: &str = "TestProcedure";

    pub fn uri(namespace: &str, type_name: &str) -> String {
        format!("{}{}", namespace, type_name)
    }
}

// --- STRUCTURES ---
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PropertyType {
    DatatypeProperty,
    ObjectProperty,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Class {
    pub iri: String,
    pub label: String,
    pub comment: String,
    pub sub_class_of: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Property {
    pub iri: String,
    pub label: String,
    pub property_type: PropertyType,
    pub domain: Option<String>,
    pub range: Option<String>,
}

// ============================================================================
// DÉFINITIONS DES MODULES MÉTIERS
// ============================================================================

pub mod oa {
    use super::*;
    pub const OPERATIONAL_ACTIVITY: &str = "OperationalActivity";
    pub const OPERATIONAL_CAPABILITY: &str = "OperationalCapability";
    pub const OPERATIONAL_ACTOR: &str = "OperationalActor";
    pub const OPERATIONAL_ENTITY: &str = "OperationalEntity";

    pub fn classes() -> Vec<Class> {
        vec![
            Class {
                iri: format!("{}{}", namespaces::OA, OPERATIONAL_ACTIVITY),
                label: "Operational Activity".to_string(),
                comment: "An activity performed by an operational entity".to_string(),
                sub_class_of: None,
            },
            Class {
                iri: format!("{}{}", namespaces::OA, OPERATIONAL_CAPABILITY),
                label: "Operational Capability".to_string(),
                comment: "An ability of an organization to provide a service".to_string(),
                sub_class_of: None,
            },
            Class {
                iri: format!("{}{}", namespaces::OA, OPERATIONAL_ACTOR),
                label: "Operational Actor".to_string(),
                comment: "An entity interacting with the system".to_string(),
                sub_class_of: None,
            },
            Class {
                iri: format!("{}{}", namespaces::OA, OPERATIONAL_ENTITY),
                label: "Operational Entity".to_string(),
                comment: "An organization or group of actors".to_string(),
                sub_class_of: None,
            },
        ]
    }

    pub fn properties() -> Vec<Property> {
        vec![Property {
            iri: format!("{}involvesActivity", namespaces::OA),
            label: "involves activity".to_string(),
            property_type: PropertyType::ObjectProperty,
            domain: Some(format!("{}{}", namespaces::OA, OPERATIONAL_CAPABILITY)),
            range: Some(format!("{}{}", namespaces::OA, OPERATIONAL_ACTIVITY)),
        }]
    }
}

pub mod sa {
    use super::*;
    pub const SYSTEM_FUNCTION: &str = "SystemFunction";
    pub const SYSTEM_COMPONENT: &str = "SystemComponent";
    pub const SYSTEM_ACTOR: &str = "SystemActor";
    pub const SYSTEM_CAPABILITY: &str = "SystemCapability";

    pub fn classes() -> Vec<Class> {
        vec![
            Class {
                iri: format!("{}{}", namespaces::SA, SYSTEM_FUNCTION),
                label: "System Function".to_string(),
                comment: "A function performed by the system".to_string(),
                sub_class_of: None,
            },
            Class {
                iri: format!("{}{}", namespaces::SA, SYSTEM_COMPONENT),
                label: "System Component".to_string(),
                comment: "A component of the system".to_string(),
                sub_class_of: None,
            },
            Class {
                iri: format!("{}{}", namespaces::SA, SYSTEM_ACTOR),
                label: "System Actor".to_string(),
                comment: "External actor interacting with the system".to_string(),
                sub_class_of: None,
            },
            Class {
                iri: format!("{}{}", namespaces::SA, SYSTEM_CAPABILITY),
                label: "System Capability".to_string(),
                comment: "Ability of the system".to_string(),
                sub_class_of: None,
            },
        ]
    }

    pub fn properties() -> Vec<Property> {
        vec![]
    }
}

pub mod la {
    use super::*;
    pub const LOGICAL_COMPONENT: &str = "LogicalComponent";
    pub const LOGICAL_FUNCTION: &str = "LogicalFunction";
    pub const LOGICAL_ACTOR: &str = "LogicalActor";
    pub const LOGICAL_INTERFACE: &str = "LogicalInterface";

    pub fn classes() -> Vec<Class> {
        vec![
            Class {
                iri: format!("{}{}", namespaces::LA, LOGICAL_COMPONENT),
                label: "Logical Component".to_string(),
                comment: "A logical component".to_string(),
                sub_class_of: None,
            },
            Class {
                iri: format!("{}{}", namespaces::LA, LOGICAL_FUNCTION),
                label: "Logical Function".to_string(),
                comment: "A function in logical architecture".to_string(),
                sub_class_of: None,
            },
            Class {
                iri: format!("{}{}", namespaces::LA, LOGICAL_ACTOR),
                label: "Logical Actor".to_string(),
                comment: "Actor in logical architecture".to_string(),
                sub_class_of: None,
            },
            Class {
                iri: format!("{}{}", namespaces::LA, LOGICAL_INTERFACE),
                label: "Logical Interface".to_string(),
                comment: "Interface definition".to_string(),
                sub_class_of: None,
            },
        ]
    }
    pub fn properties() -> Vec<Property> {
        vec![]
    }
}

pub mod pa {
    use super::*;
    pub const PHYSICAL_COMPONENT: &str = "PhysicalComponent";
    pub const PHYSICAL_FUNCTION: &str = "PhysicalFunction";
    pub const PHYSICAL_ACTOR: &str = "PhysicalActor";
    pub const PHYSICAL_LINK: &str = "PhysicalLink";

    pub fn classes() -> Vec<Class> {
        vec![
            Class {
                iri: format!("{}{}", namespaces::PA, PHYSICAL_COMPONENT),
                label: "Physical Component".to_string(),
                comment: "Node or Behavior component".to_string(),
                sub_class_of: None,
            },
            Class {
                iri: format!("{}{}", namespaces::PA, PHYSICAL_FUNCTION),
                label: "Physical Function".to_string(),
                comment: "Function deployed on hardware".to_string(),
                sub_class_of: None,
            },
            Class {
                iri: format!("{}{}", namespaces::PA, PHYSICAL_ACTOR),
                label: "Physical Actor".to_string(),
                comment: "Physical entity interacting with the system".to_string(),
                sub_class_of: None,
            },
            Class {
                iri: format!("{}{}", namespaces::PA, PHYSICAL_LINK),
                label: "Physical Link".to_string(),
                comment: "Cable, Bus, or Wireless connection".to_string(),
                sub_class_of: None,
            },
        ]
    }
    pub fn properties() -> Vec<Property> {
        vec![]
    }
}

pub mod epbs {
    use super::*;
    pub const CONFIGURATION_ITEM: &str = "ConfigurationItem";

    pub fn classes() -> Vec<Class> {
        vec![Class {
            iri: format!("{}{}", namespaces::EPBS, CONFIGURATION_ITEM),
            label: "Configuration Item".to_string(),
            comment: "Element of configuration (HWCI, CSCI)".to_string(),
            sub_class_of: None,
        }]
    }
    pub fn properties() -> Vec<Property> {
        vec![]
    }
}

pub mod data {
    use super::*;
    pub const CLASS: &str = "Class";
    pub const DATA_TYPE: &str = "DataType";
    pub const EXCHANGE_ITEM: &str = "ExchangeItem";

    pub fn classes() -> Vec<Class> {
        vec![
            Class {
                iri: format!("{}{}", namespaces::DATA, CLASS),
                label: "Data Class".to_string(),
                comment: "A complex data structure with attributes".to_string(),
                sub_class_of: None,
            },
            Class {
                iri: format!("{}{}", namespaces::DATA, DATA_TYPE),
                label: "Data Type".to_string(),
                comment: "Primitive type or enumeration".to_string(),
                sub_class_of: None,
            },
            Class {
                iri: format!("{}{}", namespaces::DATA, EXCHANGE_ITEM),
                label: "Exchange Item".to_string(),
                comment: "An element exchanged between functions".to_string(),
                sub_class_of: None,
            },
        ]
    }
    pub fn properties() -> Vec<Property> {
        vec![]
    }
}

pub mod transverse {
    use super::*;
    // Définition basée sur la structure de fichiers réelle
    pub const REQUIREMENT: &str = "Requirement";
    pub const SCENARIO: &str = "Scenario";
    pub const FUNCTIONAL_CHAIN: &str = "FunctionalChain";
    pub const CONSTRAINT: &str = "Constraint";
    pub const TEST_PROCEDURE: &str = "TestProcedure";

    pub fn classes() -> Vec<Class> {
        vec![
            Class {
                iri: format!("{}{}", namespaces::TRANSVERSE, REQUIREMENT),
                label: "Requirement".to_string(),
                comment: "A system requirement".to_string(),
                sub_class_of: None,
            },
            Class {
                iri: format!("{}{}", namespaces::TRANSVERSE, SCENARIO),
                label: "Scenario".to_string(),
                comment: "Interaction scenario".to_string(),
                sub_class_of: None,
            },
            Class {
                iri: format!("{}{}", namespaces::TRANSVERSE, FUNCTIONAL_CHAIN),
                label: "Functional Chain".to_string(),
                comment: "A path through functions".to_string(),
                sub_class_of: None,
            },
            Class {
                iri: format!("{}{}", namespaces::TRANSVERSE, CONSTRAINT),
                label: "Constraint".to_string(),
                comment: "A system constraint".to_string(),
                sub_class_of: None,
            },
            Class {
                iri: format!("{}{}", namespaces::TRANSVERSE, TEST_PROCEDURE),
                label: "Test Procedure".to_string(),
                comment: "A verification procedure".to_string(),
                sub_class_of: None,
            },
        ]
    }
    pub fn properties() -> Vec<Property> {
        vec![]
    }
}

// --- REGISTRE PRINCIPAL (SINGLETON DYNAMIQUE) ---

static INSTANCE: OnceLock<VocabularyRegistry> = OnceLock::new();

pub struct VocabularyRegistry {
    classes: HashMap<String, Class>,
    properties: HashMap<String, Property>,
    default_context: HashMap<String, String>,
    layer_contexts: Arc<RwLock<HashMap<String, Value>>>,
}

impl Default for VocabularyRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl VocabularyRegistry {
    pub fn global() -> &'static Self {
        INSTANCE.get_or_init(Self::new)
    }

    pub fn new() -> Self {
        let mut registry = Self {
            classes: HashMap::new(),
            properties: HashMap::new(),
            default_context: HashMap::new(),
            layer_contexts: Arc::new(RwLock::new(HashMap::new())),
        };

        // Enregistrement des modules
        registry.register_module_oa();
        registry.register_module_sa();
        registry.register_module_la();
        registry.register_module_pa();
        registry.register_module_epbs();
        registry.register_module_data();
        registry.register_module_transverse();

        registry.init_default_context();
        registry
    }

    fn init_default_context(&mut self) {
        let mut map = HashMap::new();
        map.insert("arcadia".to_string(), namespaces::ARCADIA.to_string());
        map.insert("oa".to_string(), namespaces::OA.to_string());
        map.insert("sa".to_string(), namespaces::SA.to_string());
        map.insert("la".to_string(), namespaces::LA.to_string());
        map.insert("pa".to_string(), namespaces::PA.to_string());
        map.insert("epbs".to_string(), namespaces::EPBS.to_string());
        map.insert("data".to_string(), namespaces::DATA.to_string());
        map.insert("transverse".to_string(), namespaces::TRANSVERSE.to_string());

        map.insert("rdf".to_string(), namespaces::RDF.to_string());
        map.insert("rdfs".to_string(), namespaces::RDFS.to_string());
        map.insert("xsd".to_string(), namespaces::XSD.to_string());
        map.insert("dcterms".to_string(), namespaces::DCTERMS.to_string());
        map.insert("prov".to_string(), namespaces::PROV.to_string());

        self.default_context = map;
    }

    fn register_module_oa(&mut self) {
        for cls in oa::classes() {
            self.classes.insert(cls.iri.clone(), cls);
        }
        for prop in oa::properties() {
            self.properties.insert(prop.iri.clone(), prop);
        }
    }
    fn register_module_sa(&mut self) {
        for cls in sa::classes() {
            self.classes.insert(cls.iri.clone(), cls);
        }
        for prop in sa::properties() {
            self.properties.insert(prop.iri.clone(), prop);
        }
    }
    fn register_module_la(&mut self) {
        for cls in la::classes() {
            self.classes.insert(cls.iri.clone(), cls);
        }
        for prop in la::properties() {
            self.properties.insert(prop.iri.clone(), prop);
        }
    }
    fn register_module_pa(&mut self) {
        for cls in pa::classes() {
            self.classes.insert(cls.iri.clone(), cls);
        }
        for prop in pa::properties() {
            self.properties.insert(prop.iri.clone(), prop);
        }
    }
    fn register_module_epbs(&mut self) {
        for cls in epbs::classes() {
            self.classes.insert(cls.iri.clone(), cls);
        }
        for prop in epbs::properties() {
            self.properties.insert(prop.iri.clone(), prop);
        }
    }
    fn register_module_data(&mut self) {
        for cls in data::classes() {
            self.classes.insert(cls.iri.clone(), cls);
        }
        for prop in data::properties() {
            self.properties.insert(prop.iri.clone(), prop);
        }
    }
    fn register_module_transverse(&mut self) {
        for cls in transverse::classes() {
            self.classes.insert(cls.iri.clone(), cls);
        }
        for prop in transverse::properties() {
            self.properties.insert(prop.iri.clone(), prop);
        }
    }

    pub fn load_layer_from_file(&self, layer: &str, path: &Path) -> AnyResult<(), String> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| format!("Impossible de lire le fichier {}: {}", path.display(), e))?;

        let json: Value = json::parse(&content)
            .map_err(|e| format!("JSON-LD invalide dans {}: {}", path.display(), e))?;

        if let Some(ctx) = json.get("@context") {
            let mut cache = self.layer_contexts.write().map_err(|e| e.to_string())?;
            cache.insert(layer.to_string(), ctx.clone());
            #[cfg(debug_assertions)]
            println!("✅ Ontologie chargée : {} -> {:?}", layer, path);
        } else {
            return Err(format!("Pas de champ @context dans {}", path.display()));
        }
        Ok(())
    }

    pub fn get_context_for_layer(&self, layer: &str) -> Option<Value> {
        let cache = self.layer_contexts.read().ok()?;
        cache.get(layer).cloned()
    }

    // --- ACCESSEURS OPTIMISÉS ---

    pub fn get_class(&self, iri: &str) -> Option<&Class> {
        self.classes.get(iri)
    }

    pub fn has_class(&self, iri: &str) -> bool {
        self.classes.contains_key(iri)
    }

    pub fn get_property(&self, iri: &str) -> Option<&Property> {
        self.properties.get(iri)
    }

    pub fn is_subtype_of(&self, child_iri: &str, parent_iri: &str) -> bool {
        if child_iri == parent_iri {
            return true;
        }
        if let Some(cls) = self.classes.get(child_iri) {
            if let Some(parent) = &cls.sub_class_of {
                return self.is_subtype_of(parent, parent_iri);
            }
        }
        false
    }

    pub fn get_default_context(&self) -> &HashMap<String, String> {
        &self.default_context
    }

    pub fn is_iri(term: &str) -> bool {
        term.starts_with("http://") || term.starts_with("https://") || term.starts_with("urn:")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_namespaces() {
        assert_eq!(namespaces::ARCADIA, "https://raise.io/ontology/arcadia#");
        assert_eq!(
            namespaces::TRANSVERSE,
            "https://raise.io/ontology/arcadia/transverse#"
        );
    }

    #[test]
    fn test_get_property_attributes() {
        let reg = VocabularyRegistry::global();
        let prop_iri = format!("{}involvesActivity", namespaces::OA);

        let prop = reg
            .get_property(&prop_iri)
            .expect("La propriété OA devrait exister");

        // Validation sémantique du domaine et du range
        assert!(prop
            .domain
            .as_ref()
            .unwrap()
            .contains("OperationalCapability"));
        assert!(prop.range.as_ref().unwrap().contains("OperationalActivity"));
        assert_eq!(prop.property_type, PropertyType::ObjectProperty);
    }

    #[test]
    fn test_is_subtype_reflexive() {
        let reg = VocabularyRegistry::global();
        let actor = format!("{}{}", namespaces::OA, arcadia_types::OA_ACTOR);
        assert!(reg.is_subtype_of(&actor, &actor));
    }

    #[test]
    fn test_inheritance_logic() {
        let mut reg = VocabularyRegistry::new();

        let parent_iri = "http://test.org/Animal".to_string();
        let child_iri = "http://test.org/Chat".to_string();

        reg.classes.insert(
            parent_iri.clone(),
            Class {
                iri: parent_iri.clone(),
                label: "Animal".into(),
                comment: "".into(),
                sub_class_of: None,
            },
        );

        reg.classes.insert(
            child_iri.clone(),
            Class {
                iri: child_iri.clone(),
                label: "Chat".into(),
                comment: "".into(),
                sub_class_of: Some(parent_iri.clone()),
            },
        );

        assert!(reg.is_subtype_of(&child_iri, &parent_iri));
        assert!(!reg.is_subtype_of(&parent_iri, &child_iri));
    }

    #[test]
    fn test_transverse_module() {
        let reg = VocabularyRegistry::global();
        // Modification pour coller aux vrais types (Requirement, Scenario...)
        let req = format!(
            "{}{}",
            namespaces::TRANSVERSE,
            arcadia_types::TRANSVERSE_REQUIREMENT
        );
        assert!(reg.has_class(&req));
        assert!(reg.get_default_context().contains_key("transverse"));
    }

    #[test]
    fn test_load_errors() {
        let reg = VocabularyRegistry::new();
        let path = Path::new("inconnu.jsonld");
        let res = reg.load_layer_from_file("test", path);
        assert!(res.is_err());
        assert!(res.unwrap_err().contains("Impossible de lire"));
    }
}
