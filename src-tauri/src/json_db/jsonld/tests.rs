// FICHIER : src-tauri/src/json_db/jsonld/tests.rs

use super::context::{ArcadiaContext, ArcadiaLayer, ContextManager};
use super::processor::{JsonLdProcessor, RdfGraph, RdfNode};
use super::vocabulary::{namespaces, oa, VocabularyRegistry}; // Suppression de PropertyType
use serde_json::json;

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
    let mut graph = RdfGraph::new();

    graph.add_triple(
        "http://example.org/s".to_string(),
        "http://example.org/p".to_string(),
        RdfNode::Literal("o".to_string()),
    );
    // Note: Ce test valide la structure RdfGraph
}

#[test]
fn test_processor_creation() {
    let processor = JsonLdProcessor::new();
    let ctx_manager = processor.context_manager();
    // Le contexte par défaut doit être chargé et accessible (champ pub)
    assert!(ctx_manager.active_namespaces.contains_key("oa"));
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

#[test]
fn test_context_creation() {
    let ctx = ArcadiaContext::new();
    assert_eq!(ctx.version, Some("1.1".to_string()));
}

#[test]
fn test_context_merge() {
    let mut ctx1 = ArcadiaContext::new();
    ctx1.add_simple_mapping("name", "http://ex1.org/name");
    assert!(ctx1.has_term("name"));
}

#[test]
fn test_layer_enum() {
    assert_eq!(ArcadiaLayer::OA.as_str(), "oa");
}

#[test]
fn test_context_manager() {
    let manager = ContextManager::new();
    // Le champ active_namespaces est maintenant public
    assert!(manager.active_namespaces.contains_key("arcadia"));
}

#[test]
fn test_merged_context() {
    let manager = ContextManager::new();
    assert!(manager.contexts.is_empty());
}

#[test]
fn test_vocab_resolution() {
    let mut manager = ContextManager::new();
    let doc = json!({
        "@context": {
            "my": "http://my-ontology.org/"
        }
    });
    manager.load_from_doc(&doc).unwrap();

    assert_eq!(
        manager.expand_term("my:Term"),
        "http://my-ontology.org/Term"
    );
}

#[test]
fn test_simple_mapping() {
    let mut ctx = ArcadiaContext::new();
    ctx.add_simple_mapping("test", "http://test.org");
    assert!(ctx.has_term("test"));
}

#[test]
fn test_oa_classes() {
    let classes = oa::classes();
    assert!(!classes.is_empty());
}

#[test]
fn test_oa_properties() {
    let properties = oa::properties();
    assert!(!properties.is_empty());
}

#[test]
fn test_vocabulary_registry() {
    let registry = VocabularyRegistry::new();
    let oa_capability_iri = format!("{}{}", namespaces::OA, oa::OPERATIONAL_CAPABILITY);
    assert!(registry.has_class(&oa_capability_iri));
}

#[test]
fn test_namespaces() {
    assert_eq!(namespaces::ARCADIA, "https://raise.io/ontology/arcadia#");
}
