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
    pub fn get_primary_type(&self, doc: &JsonValue) -> Option<String> {
        self.get_types(doc).into_iter().next()
    }

    pub fn with_context_manager(context_manager: ContextManager) -> Self {
        Self { context_manager }
    }

    pub fn with_doc_context(mut self, doc: &JsonValue) -> RaiseResult<Self> {
        self.context_manager.load_from_doc(doc)?;
        Ok(self)
    }

    /// Charge le contexte d'une couche spécifique (OA, SA...) pour la résolution sémantique.
    pub fn load_layer_context(&mut self, layer: &str) -> RaiseResult<()> {
        self.context_manager.load_layer_context(layer)
    }

    pub fn context_manager(&self) -> &ContextManager {
        &self.context_manager
    }

    // =========================================================================
    // ALGORITHMES JSON-LD IN-PLACE (ZÉRO ALLOCATION PROFONDE)
    // =========================================================================

    /// Étend le document en place.
    pub fn expand_in_place(&self, doc: &mut JsonValue) {
        match doc {
            JsonValue::Object(map) => {
                let mut keys_to_replace = Vec::new();

                for (k, v) in map.iter_mut() {
                    if k == "@context" {
                        continue;
                    }
                    let expanded_key = self.context_manager.expand_term(k);

                    if expanded_key != *k {
                        keys_to_replace.push((k.clone(), expanded_key.clone()));
                    }

                    if expanded_key == "@type" || k == "@type" {
                        self.expand_value_as_iri_in_place(v);
                    } else {
                        self.expand_in_place(v);
                    }
                }

                for (old_key, new_key) in keys_to_replace {
                    if let Some(val) = map.remove(&old_key) {
                        map.insert(new_key, val);
                    }
                }
            }
            JsonValue::Array(arr) => {
                for v in arr.iter_mut() {
                    self.expand_in_place(v);
                }
            }
            _ => {}
        }
    }

    /// Compacte le document en place.
    pub fn compact_in_place(&self, doc: &mut JsonValue) {
        match doc {
            JsonValue::Object(map) => {
                let mut keys_to_replace = Vec::new();

                for (k, v) in map.iter_mut() {
                    if k == "@context" {
                        continue;
                    }

                    let compacted_key = self.context_manager.compact_iri(k);

                    if compacted_key != *k {
                        keys_to_replace.push((k.clone(), compacted_key.clone()));
                    }

                    if compacted_key == "@type" || k == "@type" {
                        self.compact_value_as_iri_in_place(v);
                    } else {
                        self.compact_in_place(v);
                    }
                }

                for (old_key, new_key) in keys_to_replace {
                    if let Some(val) = map.remove(&old_key) {
                        map.insert(new_key, val);
                    }
                }
            }
            JsonValue::Array(arr) => {
                for v in arr.iter_mut() {
                    self.compact_in_place(v);
                }
            }
            _ => {}
        }
    }

    fn expand_value_as_iri_in_place(&self, val: &mut JsonValue) {
        match val {
            JsonValue::String(s) => {
                *s = self.context_manager.expand_term(s);
            }
            JsonValue::Array(arr) => {
                for v in arr.iter_mut() {
                    self.expand_value_as_iri_in_place(v);
                }
            }
            _ => {}
        }
    }

    fn compact_value_as_iri_in_place(&self, val: &mut JsonValue) {
        match val {
            JsonValue::String(s) => {
                *s = self.context_manager.compact_iri(s);
            }
            JsonValue::Array(arr) => {
                for v in arr.iter_mut() {
                    self.compact_value_as_iri_in_place(v);
                }
            }
            _ => {}
        }
    }

    // =========================================================================
    // UTILITAIRES RDF / VALIDATION
    // =========================================================================

    pub fn get_id(&self, doc: &JsonValue) -> Option<String> {
        doc.get("@id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
    }

    pub fn get_types(&self, doc: &JsonValue) -> Vec<String> {
        let mut types = Vec::new();

        let extract_types = |val: &JsonValue, out: &mut Vec<String>| {
            if let Some(s) = val.as_str() {
                out.push(s.to_string());
            } else if let Some(arr) = val.as_array() {
                for item in arr {
                    if let Some(s) = item.as_str() {
                        out.push(s.to_string());
                    }
                }
            }
        };

        if let Some(t) = doc.get("@type") {
            extract_types(t, &mut types);
        }
        if let Some(t) = doc.get("http://www.w3.org/1999/02/22-rdf-syntax-ns#type") {
            extract_types(t, &mut types);
        }

        types
    }

    pub fn validate_required_fields(&self, doc: &JsonValue, required: &[&str]) -> RaiseResult<()> {
        for &field in required {
            let required_iri = self.context_manager.expand_term(field);
            let mut found = false;

            if let Some(obj) = doc.as_object() {
                for (key, _) in obj {
                    let expanded_key = self.context_manager.expand_term(key);
                    if expanded_key == required_iri {
                        found = true;
                        break;
                    }
                }
            }

            if !found {
                raise_error!(
                    "ERR_SEMANTIC_FIELD_MISSING",
                    error = format!("Champ requis '{}' introuvable.", field),
                    context = json_value!({
                        "action": "VALIDATE_REQUIRED_FIELDS",
                        "field_name": field,
                        "iri_target": required_iri
                    })
                );
            }
        }
        Ok(())
    }

    pub fn to_ntriples(&self, doc: &mut JsonValue) -> RaiseResult<String> {
        self.expand_in_place(doc);

        let id = match self.get_id(doc) {
            Some(id_str) => id_str,
            None => raise_error!(
                "ERR_SEMANTIC_ID_MISSING",
                error = "Identifiant sémantique '@id' introuvable après expansion.",
                context = json_value!({
                    "available_keys": doc.as_object().map(|m| m.keys().collect::<Vec<_>>()),
                })
            ),
        };

        let mut lines = Vec::new();

        if let Some(obj) = doc.as_object() {
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
                        JsonValue::String(s)
                            if self.context_manager.expand_term(s).starts_with("http") =>
                        {
                            format!("<{}>", self.context_manager.expand_term(s))
                        }
                        JsonValue::String(s) => format!("\"{}\"", s),
                        JsonValue::Bool(b) => {
                            format!("\"{}\"^^<http://www.w3.org/2001/XMLSchema#boolean>", b)
                        }
                        JsonValue::Number(n) => {
                            format!("\"{}\"^^<http://www.w3.org/2001/XMLSchema#double>", n)
                        }
                        _ => format!("\"{}\"", o.to_string().replace("\"", "\\\"")),
                    };
                    lines.push(format!("<{}> <{}> {} .", id, pred, obj_str));
                }
            }
        }

        Ok(lines.join("\n"))
    }
}

// ============================================================================
// TESTS UNITAIRES (Corrigés pour URIs Production)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json_db::jsonld::vocabulary::VocabularyRegistry;

    fn setup_test_processor() -> RaiseResult<JsonLdProcessor> {
        VocabularyRegistry::init_mock_for_tests();
        Ok(JsonLdProcessor::new())
    }

    #[test]
    #[serial_test::serial]
    fn test_lazy_validation_success() -> RaiseResult<()> {
        let processor = setup_test_processor()?;
        let doc = json_value!({
            "@id": "urn:uuid:123",
            "oa:name": "Mission Alpha"
        });

        // 🎯 FIX : Utilisation des URIs de production
        let full_iri = "https://raise.io/oa#name";
        processor.validate_required_fields(&doc, &["@id", "oa:name"])?;
        processor.validate_required_fields(&doc, &[full_iri])?;

        Ok(())
    }

    #[test]
    #[serial_test::serial]
    fn test_expand_in_place_zero_allocation() -> RaiseResult<()> {
        let processor = setup_test_processor()?;
        let mut doc = json_value!({
            "@id": "urn:uuid:123",
            "@type": "oa:OperationalActivity",
            "oa:name": "Surveiller Zone"
        });

        processor.expand_in_place(&mut doc);

        let obj = doc.as_object().unwrap();
        let type_val = obj.get("@type").and_then(|v| v.as_str()).unwrap();

        // 🎯 FIX : Validation contre URI simplifiée
        if type_val != "https://raise.io/oa#OperationalActivity" {
            raise_error!(
                "TEST_FAIL",
                context = json_value!({
                    "action": "TEST_EXPAND_IN_PLACE_ZERO_ALLOCATION",
                    "technical_error": format!("Expansion @type échouée: {}", type_val)
                })
            );
        }
        Ok(())
    }

    #[test]
    #[serial_test::serial]
    fn test_compact_in_place_zero_allocation() -> RaiseResult<()> {
        let processor = setup_test_processor()?;
        let mut doc = json_value!({
            "@id": "urn:uuid:123",
            "@type": "https://raise.io/oa#OperationalActivity",
            "https://raise.io/oa#name": "Surveiller Zone"
        });

        processor.compact_in_place(&mut doc);

        let obj = doc.as_object().unwrap();
        let type_val = obj.get("@type").and_then(|v| v.as_str()).unwrap();

        assert_eq!(type_val, "oa:OperationalActivity");
        assert!(obj.contains_key("oa:name"));
        Ok(())
    }
}
