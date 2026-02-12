// FICHIER : src-tauri/src/json_db/jsonld/processor.rs

//! Traitement des données JSON-LD pour Arcadia
//!
//! Ce module fournit des fonctions pour :
//! - Expansion / Compaction
//! - Normalisation RDF
//! - Validation

use super::context::ContextManager;
use crate::utils::data::Map;
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

    pub fn with_doc_context(mut self, doc: &Value) -> Result<Self> {
        self.context_manager.load_from_doc(doc)?;
        Ok(self)
    }

    /// Charge le contexte d'une couche spécifique (OA, SA...) pour la résolution sémantique.
    /// Indispensable pour que le ModelLoader puisse comprendre les types sans préfixe.
    pub fn load_layer_context(&mut self, layer: &str) -> Result<()> {
        self.context_manager.load_layer_context(layer)
    }

    pub fn context_manager(&self) -> &ContextManager {
        &self.context_manager
    }

    // --- ALGORITHMES JSON-LD ---

    pub fn expand(&self, doc: &Value) -> Value {
        match doc {
            Value::Object(map) => {
                let mut new_map = Map::new();
                for (k, v) in map {
                    let expanded_key = self.context_manager.expand_term(k);

                    let expanded_val = if k == "@type" {
                        self.expand_value_as_iri(v)
                    } else {
                        self.expand(v)
                    };
                    new_map.insert(expanded_key, expanded_val);
                }
                Value::Object(new_map)
            }
            Value::Array(arr) => Value::Array(arr.iter().map(|v| self.expand(v)).collect()),
            _ => doc.clone(),
        }
    }

    pub fn compact(&self, doc: &Value) -> Value {
        match doc {
            Value::Object(map) => {
                let mut new_map = Map::new();
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
                Value::Object(new_map)
            }
            Value::Array(arr) => Value::Array(arr.iter().map(|v| self.compact(v)).collect()),
            _ => doc.clone(),
        }
    }

    fn expand_value_as_iri(&self, val: &Value) -> Value {
        match val {
            Value::String(s) => Value::String(self.context_manager.expand_term(s)),
            Value::Array(arr) => {
                Value::Array(arr.iter().map(|v| self.expand_value_as_iri(v)).collect())
            }
            _ => val.clone(),
        }
    }

    fn compact_value_as_iri(&self, val: &Value) -> Value {
        match val {
            Value::String(s) => Value::String(self.context_manager.compact_iri(s)),
            Value::Array(arr) => {
                Value::Array(arr.iter().map(|v| self.compact_value_as_iri(v)).collect())
            }
            _ => val.clone(),
        }
    }

    // --- UTILITAIRES RDF / VALIDATION ---

    pub fn get_id(&self, doc: &Value) -> Option<String> {
        doc.get("@id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
    }

    pub fn get_type(&self, doc: &Value) -> Option<String> {
        if let Some(t) = doc.get("@type") {
            return t.as_str().map(|s| s.to_string());
        }
        if let Some(t) = doc.get("http://www.w3.org/1999/02/22-rdf-syntax-ns#type") {
            return t.as_str().map(|s| s.to_string());
        }
        None
    }

    pub fn validate_required_fields(&self, doc: &Value, required: &[&str]) -> Result<()> {
        let expanded = self.expand(doc);
        for &field in required {
            let iri = self.context_manager.expand_term(field);
            if expanded.get(&iri).is_none() && doc.get(field).is_none() {
                return Err(AppError::NotFound(format!(
                    "Champ requis manquant : {}",
                    field
                )));
            }
        }
        Ok(())
    }

    pub fn to_ntriples(&self, doc: &Value) -> Result<String> {
        let expanded = self.expand(doc);
        let id = self
            .get_id(&expanded)
            .ok_or_else(|| AppError::Validation("Document sans @id".to_string()))?;

        let mut lines = Vec::new();

        if let Some(obj) = expanded.as_object() {
            for (pred, val) in obj {
                if pred.starts_with('@') {
                    continue;
                }

                let objects = if let Value::Array(arr) = val {
                    arr.iter().collect()
                } else {
                    vec![val]
                };

                for o in objects {
                    let obj_str = match o {
                        Value::String(s) if s.starts_with("http") => format!("<{}>", s),
                        Value::String(s) => format!("{:?}", s),
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
    use crate::utils::json::json;

    #[test]
    fn test_get_id() {
        let processor = JsonLdProcessor::new();
        let doc = json!({
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
        let doc = json!({
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
        let doc = json!({
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
        let doc = json!({
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
