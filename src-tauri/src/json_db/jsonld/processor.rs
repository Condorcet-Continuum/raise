// FICHIER : src-tauri/src/json_db/jsonld/processor.rs

//! Traitement des données JSON-LD pour Arcadia
//!
//! Ce module fournit des fonctions pour :
//! - Expansion / Compaction
//! - Normalisation RDF
//! - Validation

use super::context::ContextManager;
use super::vocabulary::VocabularyRegistry;
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

impl JsonLdProcessor {
    pub fn new() -> RaiseResult<Self> {
        Ok(Self {
            context_manager: ContextManager::new()?,
        })
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
        let type_iri = self.context_manager.expand_term("@type"); // Normalisation

        if let Some(obj) = doc.as_object() {
            for (key, val) in obj {
                // On vérifie si la clé (compacte ou étendue) correspond à @type
                if key == "@type" || self.context_manager.expand_term(key) == type_iri {
                    if let Some(s) = val.as_str() {
                        types.push(s.to_string());
                    } else if let Some(arr) = val.as_array() {
                        types.extend(arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())));
                    }
                }
            }
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
        // 1. Normalisation sémantique : On s'assure que toutes les clés sont des IRIs.
        self.expand_in_place(doc);

        // 2. Identification du sujet : Support des "Blank Nodes" si @id est absent.
        // Contrairement à la version précédente, on ne lève plus d'erreur si l'ID manque.
        let subject = self
            .get_id(doc)
            .map(|id| format!("<{}>", id))
            .unwrap_or_else(|| "_:b0".to_string());

        let mut lines = Vec::new();

        if let Some(obj) = doc.as_object() {
            for (pred, val) in obj {
                // On ignore les mots-clés système (@id, @context, etc.) pour le prédicat.
                if pred.starts_with('@') {
                    continue;
                }

                // Normalisation du prédicat : Après expansion, il doit être wrappé en <IRI>.
                let pred_iri = if pred.starts_with("http") || pred.contains(':') {
                    format!("<{}>", pred)
                } else {
                    format!("<{}>", self.context_manager.expand_term(pred))
                };

                // Gestion de l'atomicité : on traite les valeurs simples et les tableaux.
                let objects = match val {
                    JsonValue::Array(arr) => arr.iter().collect::<Vec<_>>(),
                    _ => vec![val],
                };

                for o in objects {
                    let obj_str = match o {
                        // Cas 1 : La valeur est une IRI (Ressource liée).
                        JsonValue::String(s) if VocabularyRegistry::is_iri(s) => {
                            format!("<{}>", s)
                        }
                        // Cas 2 : Littéral simple avec échappement des guillemets.
                        JsonValue::String(s) => format!("\"{}\"", s.replace('\"', "\\\"")),
                        // Cas 3 : Types Primitifs avec correspondance XSD stricte.
                        JsonValue::Bool(b) => {
                            format!("\"{}\"^^<http://www.w3.org/2001/XMLSchema#boolean>", b)
                        }
                        JsonValue::Number(n) => {
                            let xsd_type = if n.is_f64() { "double" } else { "integer" };
                            format!("\"{}\"^^<http://www.w3.org/2001/XMLSchema#{}>", n, xsd_type)
                        }
                        // Cas 4 : Repli pour les types complexes (Objets/Arrays) sérialisés en littéraux.
                        _ => format!("\"{}\"", o.to_string().replace('\"', "\\\"")),
                    };

                    lines.push(format!("{} {} {} .", subject, pred_iri, obj_str));
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
        JsonLdProcessor::new()
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

        let Some(obj) = doc.as_object() else {
            raise_error!(
                "TEST_FAIL",
                error = "Le document après expansion n'est pas un objet."
            );
        };
        let Some(type_val) = obj.get("@type").and_then(|v| v.as_str()) else {
            raise_error!(
                "TEST_FAIL",
                error = "Champ @type manquant ou n'est pas une chaîne."
            );
        };

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
    fn test_to_ntriples_robustness() -> RaiseResult<()> {
        let processor = setup_test_processor()?;

        // Cas 1 : Document complet avec Types Numériques
        let mut doc = json_value!({
            "@id": "urn:uuid:456",
            "oa:name": "Alpha",
            "oa:count": 10
        });

        let nt = processor.to_ntriples(&mut doc)?;
        assert!(nt.contains("<urn:uuid:456> <https://raise.io/oa#name> \"Alpha\" ."));
        // Vérification du type XSD integer
        assert!(nt.contains("\"10\"^^<http://www.w3.org/2001/XMLSchema#integer>"));

        // Cas 2 : Document sans @id (Support des Blank Nodes)
        let mut doc_anon = json_value!({
            "oa:name": "Anonyme"
        });
        let nt_anon = processor.to_ntriples(&mut doc_anon)?;
        assert!(
            nt_anon.starts_with("_:b0"),
            "Devrait générer un Blank Node pour un doc sans ID."
        );

        Ok(())
    }
}
