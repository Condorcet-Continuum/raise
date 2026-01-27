// FICHIER : src-tauri/src/json_db/jsonld/vocabulary.rs

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::{Arc, OnceLock, RwLock};

// --- NAMESPACES ---
pub mod namespaces {
    pub const ARCADIA: &str = "https://raise.io/ontology/arcadia#";
    pub const OA: &str = "https://raise.io/ontology/arcadia/oa#";
    pub const SA: &str = "https://raise.io/ontology/arcadia/sa#";
    pub const LA: &str = "https://raise.io/ontology/arcadia/la#";
    pub const PA: &str = "https://raise.io/ontology/arcadia/pa#";
    pub const EPBS: &str = "https://raise.io/ontology/arcadia/epbs#";
    pub const DATA: &str = "https://raise.io/ontology/arcadia/data#";

    // Standards
    pub const RDF: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#";
    pub const RDFS: &str = "http://www.w3.org/2000/01/rdf-schema#";
    pub const OWL: &str = "http://www.w3.org/2002/07/owl#";
    pub const XSD: &str = "http://www.w3.org/2001/XMLSchema#";
    pub const DCTERMS: &str = "http://purl.org/dc/terms/";
    pub const PROV: &str = "http://www.w3.org/ns/prov#";
}

// --- CONSTANTES DE TYPAGE (RESTAURÉES) ---
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
// DÉFINITIONS DES MODULES MÉTIERS (Pour validation interne)
// ============================================================================

pub mod oa {
    use super::*;
    // Ces constantes sont aussi utiles ici pour les définitions internes
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
}

// --- REGISTRE PRINCIPAL (SINGLETON DYNAMIQUE) ---

static INSTANCE: OnceLock<VocabularyRegistry> = OnceLock::new();

pub struct VocabularyRegistry {
    classes: HashMap<String, Class>,
    properties: HashMap<String, Property>,
    default_context: HashMap<String, String>,

    // CACHE DYNAMIQUE : Stocke les contextes chargés depuis les fichiers .jsonld
    // Arc<RwLock> permet la mutabilité même si le registre est statique (Singleton)
    layer_contexts: Arc<RwLock<HashMap<String, Value>>>,
}

impl Default for VocabularyRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl VocabularyRegistry {
    /// Accès global Thread-Safe au registre (créé une seule fois)
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

        // Enregistrement des définitions "hardcodées" (Validation structurelle)
        registry.register_module_oa();
        registry.register_module_sa();
        registry.register_module_la();
        registry.register_module_pa();
        registry.register_module_epbs();
        registry.register_module_data();

        // Initialisation des préfixes par défaut
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
    }

    fn register_module_la(&mut self) {
        for cls in la::classes() {
            self.classes.insert(cls.iri.clone(), cls);
        }
    }

    fn register_module_pa(&mut self) {
        for cls in pa::classes() {
            self.classes.insert(cls.iri.clone(), cls);
        }
    }

    fn register_module_epbs(&mut self) {
        for cls in epbs::classes() {
            self.classes.insert(cls.iri.clone(), cls);
        }
    }

    fn register_module_data(&mut self) {
        for cls in data::classes() {
            self.classes.insert(cls.iri.clone(), cls);
        }
    }

    // --- CHARGEMENT DYNAMIQUE (.jsonld) ---

    /// Charge un fichier .jsonld pour une couche donnée (ex: "oa", "sa")
    pub fn load_layer_from_file(&self, layer: &str, path: &Path) -> Result<(), String> {
        let content = fs::read_to_string(path)
            .map_err(|e| format!("Impossible de lire le fichier {}: {}", path.display(), e))?;

        let json: Value = serde_json::from_str(&content)
            .map_err(|e| format!("JSON-LD invalide dans {}: {}", path.display(), e))?;

        // On extrait le bloc @context du fichier JSON-LD
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

    /// Récupère le contexte complet (JSON) pour une couche donnée
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
    }

    #[test]
    fn test_oa_classes() {
        let classes = oa::classes();
        assert!(!classes.is_empty());
    }

    #[test]
    fn test_singleton_consistency() {
        let reg1 = VocabularyRegistry::global();
        let reg2 = VocabularyRegistry::global();
        // Vérifie que c'est bien la même adresse mémoire
        assert!(std::ptr::eq(reg1, reg2));
    }

    #[test]
    fn test_default_context_cached() {
        let reg = VocabularyRegistry::global();
        let ctx = reg.get_default_context();
        assert!(ctx.contains_key("oa"));
        assert!(ctx.contains_key("rdf"));
    }

    #[test]
    fn test_arcadia_types_constants_exist() {
        assert_eq!(arcadia_types::OA_ACTOR, "OperationalActor");
        assert_eq!(arcadia_types::SA_FUNCTION, "SystemFunction");
    }
}
