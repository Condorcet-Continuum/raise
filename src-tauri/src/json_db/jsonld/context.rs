// FICHIER : src-tauri/src/json_db/jsonld/context.rs

use crate::utils::data::HashMap;
use crate::utils::prelude::*;

use super::{vocabulary::VocabularyRegistry, ContextValue};

/// Enumération des couches Arcadia
/// Mise à jour pour inclure Data et Transverse
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ArcadiaLayer {
    OA,         // Operational Analysis
    SA,         // System Analysis
    LA,         // Logical Architecture
    PA,         // Physical Architecture
    EPBS,       // End-Product Breakdown Structure
    Data,       // Data Analysis / Class Diagrams
    Transverse, // Common elements, Libraries, Transverse Modeling
}

impl ArcadiaLayer {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::OA => "oa",
            Self::SA => "sa",
            Self::LA => "la",
            Self::PA => "pa",
            Self::EPBS => "epbs",
            Self::Data => "data",
            Self::Transverse => "transverse",
        }
    }
}

/// Représente un contexte JSON-LD complet avec métadonnées
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArcadiaContext {
    #[serde(rename = "@version", skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,

    #[serde(rename = "@vocab", skip_serializing_if = "Option::is_none")]
    pub vocab: Option<String>,

    #[serde(flatten)]
    pub mappings: HashMap<String, ContextValue>,
}

impl Default for ArcadiaContext {
    fn default() -> Self {
        Self::new()
    }
}

impl ArcadiaContext {
    pub fn new() -> Self {
        Self {
            version: Some("1.1".to_string()),
            vocab: None,
            mappings: HashMap::new(),
        }
    }

    pub fn add_simple_mapping(&mut self, term: &str, iri: &str) {
        self.mappings
            .insert(term.to_string(), ContextValue::Simple(iri.to_string()));
    }

    pub fn has_term(&self, term: &str) -> bool {
        self.mappings.contains_key(term)
    }
}

/// Gestionnaire principal de contexte pour le processeur
#[derive(Debug, Clone)]
pub struct ContextManager {
    /// Contextes spécifiques par couche (Legacy support)
    pub contexts: HashMap<ArcadiaLayer, ArcadiaContext>,
    /// Table de résolution active (Terme -> IRI)
    pub active_mappings: HashMap<String, String>,
}

impl Default for ContextManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ContextManager {
    pub fn new() -> Self {
        Self {
            contexts: HashMap::new(),
            // Initialisation avec le contexte par défaut (préfixes standards: oa, sa, data, rdf...)
            active_mappings: VocabularyRegistry::global().get_default_context().clone(),
        }
    }

    /// Charge le contexte d'une couche spécifique depuis le Registre Sémantique
    /// C'est la clé pour que le Loader puisse résoudre "Class" ou "OperationalActor" sans préfixe.
    pub fn load_layer_context(&mut self, layer: &str) -> Result<()> {
        let registry = VocabularyRegistry::global();
        if let Some(ctx_json) = registry.get_context_for_layer(layer) {
            self.parse_context_block(&ctx_json)
        } else {
            // Si le layer n'est pas chargé (ex: test unitaire ou transverse inexistant), on log mais on ne bloque pas.
            // Le contexte par défaut contient déjà les préfixes essentiels.
            Ok(())
        }
    }

    /// Charge un contexte depuis un document JSON-LD (@context)
    pub fn load_from_doc(&mut self, doc: &Value) -> Result<()> {
        if let Some(ctx) = doc.get("@context") {
            self.parse_context_block(ctx)?;
        }
        Ok(())
    }

    fn parse_context_block(&mut self, ctx: &Value) -> Result<()> {
        match ctx {
            Value::Object(map) => {
                for (key, val) in map {
                    // Cas 1 : "term": "iri"
                    if let Value::String(uri) = val {
                        self.active_mappings.insert(key.clone(), uri.clone());
                    }
                    // Cas 2 : "term": { "@id": "iri" }
                    else if let Value::Object(def) = val {
                        if let Some(Value::String(id)) = def.get("@id") {
                            self.active_mappings.insert(key.clone(), id.clone());
                        }
                    }
                }
            }
            Value::Array(arr) => {
                for item in arr {
                    self.parse_context_block(item)?;
                }
            }
            _ => {} // Ignore les autres types (références distantes string non gérées ici)
        }
        Ok(())
    }

    /// EXPANSION : Transforme "oa:Actor" ou "OperationalActor" en IRI complète
    pub fn expand_term(&self, term: &str) -> String {
        // 1. Si c'est déjà une IRI ou un mot-clé JSON-LD, on garde tel quel
        if VocabularyRegistry::is_iri(term) || term.starts_with('@') {
            return term.to_string();
        }

        // 2. Si le terme est défini explicitement dans le contexte
        // (ex: "OperationalActor" -> "oa:OperationalActor" ou directement l'IRI)
        if let Some(mapped) = self.active_mappings.get(term) {
            // Si le mapping renvoie une IRI absolue, c'est fini.
            if VocabularyRegistry::is_iri(mapped) {
                return mapped.clone();
            } else {
                // Si c'est un CURIE (ex: "oa:OperationalActor"), on ré-expand
                return self.expand_curie(mapped);
            }
        }

        // 3. Essai de résolution CURIE standard (prefix:suffix)
        self.expand_curie(term)
    }

    fn expand_curie(&self, term: &str) -> String {
        if let Some((prefix, suffix)) = term.split_once(':') {
            if let Some(base) = self.active_mappings.get(prefix) {
                return format!("{}{}", base, suffix);
            }
        }
        term.to_string()
    }

    /// COMPACTION : Transforme une IRI en terme court (si possible)
    pub fn compact_iri(&self, iri: &str) -> String {
        // Recherche inversée : on cherche le préfixe le plus long qui matche
        for (term, mapping) in &self.active_mappings {
            // On privilégie les préfixes (qui finissent par # ou /)
            if (mapping.ends_with('#') || mapping.ends_with('/')) && iri.starts_with(mapping) {
                let suffix = &iri[mapping.len()..];
                if !suffix.is_empty() {
                    return format!("{}:{}", term, suffix);
                }
            }
        }
        iri.to_string()
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::json::json;

    #[test]
    fn test_context_creation() {
        let ctx = ArcadiaContext::new();
        assert_eq!(ctx.version, Some("1.1".to_string()));
    }

    #[test]
    fn test_layer_enum_completeness() {
        // Validation exhaustive de toutes les couches Arcadia supportées
        assert_eq!(ArcadiaLayer::OA.as_str(), "oa");
        assert_eq!(ArcadiaLayer::SA.as_str(), "sa");
        assert_eq!(ArcadiaLayer::LA.as_str(), "la");
        assert_eq!(ArcadiaLayer::PA.as_str(), "pa");
        assert_eq!(ArcadiaLayer::EPBS.as_str(), "epbs");
        assert_eq!(ArcadiaLayer::Data.as_str(), "data"); // Test critique Data
        assert_eq!(ArcadiaLayer::Transverse.as_str(), "transverse"); // Test critique Transverse
    }

    #[test]
    fn test_context_manager_defaults() {
        let manager = ContextManager::new();
        // Vérifie que les namespaces par défaut sont chargés depuis le Registre
        assert!(manager.active_mappings.contains_key("oa"));
        assert!(
            manager.active_mappings.contains_key("data"),
            "Le namespace Data doit être présent par défaut"
        );
        assert!(manager
            .active_mappings
            .get("oa")
            .unwrap()
            .contains("raise.io"));
    }

    #[test]
    fn test_expand_curie() {
        let manager = ContextManager::new(); // Charge les préfixes par défaut
        assert_eq!(
            manager.expand_term("oa:OperationalActivity"),
            "https://raise.io/ontology/arcadia/oa#OperationalActivity"
        );
        assert_eq!(
            manager.expand_term("data:Class"),
            "https://raise.io/ontology/arcadia/data#Class",
            "Expansion du namespace Data échouée"
        );
    }

    #[test]
    fn test_load_from_doc() {
        let mut manager = ContextManager::new();
        let doc = json!({
            "@context": {
                "my": "http://my-ontology.org/",
                "Actor": "my:Actor"
            }
        });
        manager.load_from_doc(&doc).unwrap();

        // Test résolution simple préfixe
        assert_eq!(
            manager.expand_term("my:Thing"),
            "http://my-ontology.org/Thing"
        );

        // Test résolution mapping direct (Actor -> my:Actor -> http://...)
        let expanded = manager.expand_term("Actor");
        assert_eq!(expanded, "http://my-ontology.org/Actor");
    }

    #[test]
    fn test_compact_iri() {
        let manager = ContextManager::new();

        let iri_oa = "https://raise.io/ontology/arcadia/oa#OperationalActivity";
        assert_eq!(manager.compact_iri(iri_oa), "oa:OperationalActivity");

        let iri_data = "https://raise.io/ontology/arcadia/data#DataType";
        assert_eq!(
            manager.compact_iri(iri_data),
            "data:DataType",
            "Compaction Data échouée"
        );
    }
}
