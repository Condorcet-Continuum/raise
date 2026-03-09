// FICHIER : src-tauri/src/json_db/jsonld/processor.rs

//! Traitement des données JSON-LD pour Arcadia
//!
//! Ce module fournit des fonctions pour :
//! - Expansion / Compaction
//! - Normalisation RDF
//! - Validation

use super::context::ContextManager;
use crate::utils::prelude::*;

/// Représentation simple d'un nœud RDF pour l'export
#[derive(Debug, Clone)]
pub enum RdfNode {
    IRI(String),
    Literal(String),
    BlankNode(String),
}

/// Graphe RDF simplifié
#[derive(Debug, Default)]
pub struct RdfGraph {
    triples: Vec<(String, String, RdfNode)>,
}

impl RdfGraph {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_triple(&mut self, subject: String, predicate: String, object: RdfNode) {
        self.triples.push((subject, predicate, object));
    }

    pub fn triples(&self) -> &Vec<(String, String, RdfNode)> {
        &self.triples
    }

    pub fn subjects(&self) -> Vec<String> {
        let mut subs: Vec<String> = self.triples.iter().map(|(s, _, _)| s.clone()).collect();
        subs.sort();
        subs.dedup();
        subs
    }
}

/// Processeur JSON-LD pour les données Arcadia
#[derive(Debug, Clone)]
pub struct JsonLdProcessor {
    context_manager: ContextManager,
}

impl Default for JsonLdProcessor {
    fn default() -> Self {
        Self::new()
    }
}

impl JsonLdProcessor {
    pub fn new() -> Self {
        Self {
            context_manager: ContextManager::new(),
        }
    }

    pub fn with_context_manager(context_manager: ContextManager) -> Self {
        Self { context_manager }
    }

    pub fn with_doc_context(mut self, doc: &JsonValue) -> RaiseResult<Self> {
        self.context_manager.load_from_doc(doc)?;
        Ok(self)
    }

    /// Charge le contexte d'une couche spécifique (OA, SA...) pour la résolution sémantique.
    /// Indispensable pour que le ModelLoader puisse comprendre les types sans préfixe.
    pub fn load_layer_context(&mut self, layer: &str) -> RaiseResult<()> {
        self.context_manager.load_layer_context(layer)
    }

    pub fn context_manager(&self) -> &ContextManager {
        &self.context_manager
    }

    // --- ALGORITHMES JSON-LD ---

    pub fn expand(&self, doc: &JsonValue) -> JsonValue {
        match doc {
            JsonValue::Object(map) => {
                let mut new_map = JsonObject::new();
                for (k, v) in map {
                    let expanded_key = self.context_manager.expand_term(k);

                    let expanded_val = if k == "@type" {
                        self.expand_value_as_iri(v)
                    } else {
                        self.expand(v)
                    };
                    new_map.insert(expanded_key, expanded_val);
                }
                JsonValue::Object(new_map)
            }
            JsonValue::Array(arr) => JsonValue::Array(arr.iter().map(|v| self.expand(v)).collect()),
            _ => doc.clone(),
        }
    }

    pub fn compact(&self, doc: &JsonValue) -> JsonValue {
        match doc {
            JsonValue::Object(map) => {
                let mut new_map = JsonObject::new();
                for (k, v) in map {
                    if k == "@context" {
                        continue;
                    }

                    let compacted_key = self.context_manager.compact_iri(k);

                    let compacted_val = if k == "@type" {
                        self.compact_value_as_iri(v)
                    } else {
                        self.compact(v)
                    };
                    new_map.insert(compacted_key, compacted_val);
                }
                JsonValue::Object(new_map)
            }
            JsonValue::Array(arr) => {
                JsonValue::Array(arr.iter().map(|v| self.compact(v)).collect())
            }
            _ => doc.clone(),
        }
    }

    fn expand_value_as_iri(&self, val: &JsonValue) -> JsonValue {
        match val {
            JsonValue::String(s) => JsonValue::String(self.context_manager.expand_term(s)),
            JsonValue::Array(arr) => {
                JsonValue::Array(arr.iter().map(|v| self.expand_value_as_iri(v)).collect())
            }
            _ => val.clone(),
        }
    }

    fn compact_value_as_iri(&self, val: &JsonValue) -> JsonValue {
        match val {
            JsonValue::String(s) => JsonValue::String(self.context_manager.compact_iri(s)),
            JsonValue::Array(arr) => {
                JsonValue::Array(arr.iter().map(|v| self.compact_value_as_iri(v)).collect())
            }
            _ => val.clone(),
        }
    }

    // --- UTILITAIRES RDF / VALIDATION ---

    pub fn get_id(&self, doc: &JsonValue) -> Option<String> {
        doc.get("@id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
    }

    pub fn get_type(&self, doc: &JsonValue) -> Option<String> {
        if let Some(t) = doc.get("@type") {
            return t.as_str().map(|s| s.to_string());
        }
        if let Some(t) = doc.get("http://www.w3.org/1999/02/22-rdf-syntax-ns#type") {
            return t.as_str().map(|s| s.to_string());
        }
        None
    }

    pub fn validate_required_fields(&self, doc: &JsonValue, required: &[&str]) -> RaiseResult<()> {
        let expanded = self.expand(doc);
        for &field in required {
            let iri = self.context_manager.expand_term(field);
            if expanded.get(&iri).is_none() && doc.get(field).is_none() {
                raise_error!(
                    "ERR_SEMANTIC_FIELD_MISSING",
                    error = format!("Champ requis '{}' introuvable (recherche infructueuse dans le document et l'IRI étendu).", field),
                    context = json_value!({
                        "field_name": field,
                        "iri_target": iri,
                        "sources_checked": ["document_root", "expanded_context"],
                        "action": "validate_required_semantic_fields"
                    })
                );
            }
        }
        Ok(())
    }

    pub fn to_ntriples(&self, doc: &JsonValue) -> RaiseResult<String> {
        let expanded = self.expand(doc);
        let Some(id) = self.get_id(&expanded) else {
            raise_error!(
                "ERR_SEMANTIC_ID_MISSING",
                error = "Identifiant sémantique '@id' introuvable après expansion.",
                context = json_value!({
                    "action": "extract_semantic_id",
                    // FIX : On passe par as_object() pour accéder aux clés en toute sécurité
                    "available_keys": expanded.as_object().map(|m| m.keys().collect::<Vec<_>>()),
                    "hint": "Le document JSON-LD étendu ne contient pas de champ '@id' valide."
                })
            );
        };
        let mut lines = Vec::new();

        if let Some(obj) = expanded.as_object() {
            for (pred, val) in obj {
                if pred.starts_with('@') {
                    continue;
                }

                let objects = if let JsonValue::Array(arr) = val {
                    arr.iter().collect()
                } else {
                    vec![val]
                };

                for o in objects {
                    let obj_str = match o {
                        JsonValue::String(s) if s.starts_with("http") => format!("<{}>", s),
                        JsonValue::String(s) => format!("{:?}", s),
                        _ => format!("{:?}", o.to_string()),
                    };
                    lines.push(format!("<{}> <{}> {} .", id, pred, obj_str));
                }
            }
        }

        Ok(lines.join("\n"))
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_id() {
        let processor = JsonLdProcessor::new();
        let doc = json_value!({
            "@id": "http://example.org/1"
        });
        assert_eq!(
            processor.get_id(&doc),
            Some("http://example.org/1".to_string())
        );
    }

    #[test]
    fn test_get_type() {
        let processor = JsonLdProcessor::new();
        let doc = json_value!({
            "@type": "http://example.org/Type"
        });
        assert_eq!(
            processor.get_type(&doc),
            Some("http://example.org/Type".to_string())
        );
    }

    #[test]
    fn test_validate_required_fields() {
        let processor = JsonLdProcessor::new();
        let doc = json_value!({
            "@id": "test",
            "name": "Test Activity"
        });

        assert!(processor
            .validate_required_fields(&doc, &["@id", "name"])
            .is_ok());
        assert!(processor
            .validate_required_fields(&doc, &["@id", "name", "description"])
            .is_err());
    }

    #[test]
    fn test_rdf_graph() {
        let mut graph = RdfGraph::new();

        graph.add_triple(
            "http://example.org/activity-1".to_string(),
            "http://www.w3.org/1999/02/22-rdf-syntax-ns#type".to_string(),
            RdfNode::IRI("http://example.org/OperationalActivity".to_string()),
        );

        graph.add_triple(
            "http://example.org/activity-1".to_string(),
            "http://www.w3.org/2004/02/skos/core#prefLabel".to_string(),
            RdfNode::Literal("Test Activity".to_string()),
        );

        assert_eq!(graph.triples().len(), 2);
        assert_eq!(graph.subjects().len(), 1);
    }

    #[test]
    fn test_ntriples_export() {
        // Validation simple de la structure graph
        let mut graph = RdfGraph::new();
        graph.add_triple(
            "http://example.org/s".to_string(),
            "http://example.org/p".to_string(),
            RdfNode::Literal("o".to_string()),
        );
        assert_eq!(graph.triples().len(), 1);
    }

    #[test]
    fn test_processor_creation() {
        let processor = JsonLdProcessor::new();
        let ctx_manager = processor.context_manager();
        // Le contexte par défaut doit être chargé
        // CORRECTION : utilisation de 'active_mappings' au lieu de 'active_namespaces'
        assert!(ctx_manager.active_mappings.contains_key("oa"));
    }

    #[test]
    fn test_expand_with_oa() {
        let doc = json_value!({
            "@id": "urn:uuid:123",
            "@type": "oa:OperationalActivity",
            "oa:name": "Manger"
        });

        let processor = JsonLdProcessor::new();
        let expanded = processor.expand(&doc);
        let obj = expanded.as_object().unwrap();

        let type_val = obj.get("@type").unwrap().as_str().unwrap();
        assert!(type_val.contains("raise.io/ontology/arcadia/oa#OperationalActivity"));
    }
}
