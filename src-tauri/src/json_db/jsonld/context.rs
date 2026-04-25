// FICHIER : src-tauri/src/json_db/jsonld/context.rs

use super::{vocabulary::VocabularyRegistry, ContextJsonValue};
use crate::utils::prelude::*;

/// Couches méthodologiques supportées par l'architecture Arcadia.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serializable, Deserializable)]
#[serde(rename_all = "lowercase")]
pub enum ArcadiaLayer {
    OA,
    SA,
    LA,
    PA,
    EPBS,
    Data,
    Transverse,
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

/// Représentation structurée d'un bloc @context JSON-LD.
#[derive(Debug, Clone, Serializable, Deserializable)]
pub struct ArcadiaContext {
    #[serde(rename = "@version", skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(flatten)]
    pub mappings: UnorderedMap<String, ContextJsonValue>,
}

impl Default for ArcadiaContext {
    fn default() -> Self {
        Self {
            version: Some("1.1".to_string()),
            mappings: UnorderedMap::new(),
        }
    }
}

/// Gestionnaire de contexte haute performance.
/// Garantit une empreinte RAM minimale via le partage de pointeurs (Interning).
#[derive(Debug, Clone)]
pub struct ContextManager {
    pub contexts: UnorderedMap<ArcadiaLayer, ArcadiaContext>,
    /// Mappings actifs utilisant des SharedRef<str> pour la déduplication.
    pub active_mappings: UnorderedMap<String, SharedRef<str>>,
}

impl ContextManager {
    /// Initialise le manager avec le dictionnaire global du registre.
    pub fn new() -> RaiseResult<Self> {
        let registry = VocabularyRegistry::global()?;
        Ok(Self {
            contexts: UnorderedMap::new(),
            active_mappings: registry.get_default_context(),
        })
    }

    /// Charge le contexte d'une couche métier depuis le registre sémantique.
    pub fn load_layer_context(&mut self, layer: &str) -> RaiseResult<()> {
        let registry = VocabularyRegistry::global()?;
        if let Some(ctx_json) = registry.get_context_for_layer(layer) {
            self.parse_context_block(&ctx_json)
        } else {
            Ok(())
        }
    }

    /// Intègre les mappings d'un document spécifique dans le contexte courant.
    pub fn load_from_doc(&mut self, doc: &JsonValue) -> RaiseResult<()> {
        if let Some(ctx) = doc.get("@context") {
            self.parse_context_block(ctx)?;
        }
        Ok(())
    }

    /// Analyse récursivement les blocs de contexte et interne les URIs.
    fn parse_context_block(&mut self, ctx: &JsonValue) -> RaiseResult<()> {
        let registry = VocabularyRegistry::global()?;
        match ctx {
            JsonValue::Object(map) => {
                for (key, val) in map {
                    if let JsonValue::String(uri) = val {
                        self.active_mappings
                            .insert(key.clone(), registry.intern(uri));
                    } else if let JsonValue::Object(def) = val {
                        if let Some(id) = def.get("@id").and_then(|v| v.as_str()) {
                            self.active_mappings
                                .insert(key.clone(), registry.intern(id));
                        }
                    }
                }
            }
            JsonValue::Array(arr) => {
                for item in arr {
                    self.parse_context_block(item)?;
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// Étend un terme ou une CURIE vers une IRI absolue (Sécurisé contre la récursion).
    pub fn expand_term(&self, term: &str) -> String {
        self.expand_term_recursive(term, 0)
    }

    fn expand_term_recursive(&self, term: &str, depth: u8) -> String {
        // 🎯 Protection contre les cycles : Maximum 6 niveaux (Pair pour retrouver le terme initial)
        // L'utilisation de >= 6 garantit qu'au 6ème appel (cycle complet), on retourne le terme stable.
        if depth >= 6 || VocabularyRegistry::is_iri(term) || term.starts_with('@') {
            return term.to_string();
        }

        if let Some(mapped) = self.active_mappings.get(term) {
            let m_str = mapped.as_ref();
            if VocabularyRegistry::is_iri(m_str) {
                return m_str.to_string();
            } else {
                return self.expand_term_recursive(m_str, depth + 1);
            }
        }
        self.expand_curie(term, depth)
    }

    fn expand_curie(&self, term: &str, depth: u8) -> String {
        let registry = match VocabularyRegistry::global() {
            Ok(r) => r,
            Err(_) => return term.to_string(), // Fallback sécurisé
        };
        if let Some((prefix, suffix)) = term.split_once(':') {
            if let Some(base) = self.active_mappings.get(prefix) {
                // Résolution récursive du préfixe lui-même
                let base_iri = self.expand_term_recursive(base.as_ref(), depth + 1);
                let full_iri = format!("{}{}", base_iri, suffix);
                return registry.intern(&full_iri).to_string();
            }
        }
        term.to_string()
    }

    /// Compacte une IRI absolue vers la forme la plus courte disponible.
    pub fn compact_iri(&self, iri: &str) -> String {
        for (term, mapping) in &self.active_mappings {
            let m = mapping.as_ref();
            if (m.ends_with('#') || m.ends_with('/')) && iri.starts_with(m) {
                let suffix = &iri[m.len()..];
                if !suffix.is_empty() {
                    return format!("{}:{}", term, suffix);
                }
            }
        }
        iri.to_string()
    }
}

// ============================================================================
// TESTS UNITAIRES (Rigueur Maximale)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// 💎 TEST : Vérification de l'Interning (Ptr equality).
    #[async_test]
    #[serial_test::serial]
    async fn test_context_interning_ptr_integrity() -> RaiseResult<()> {
        crate::utils::testing::mock::inject_mock_config().await; // 🎯 FIX : Initialisation explicite

        let manager = ContextManager::new()?;
        let registry = VocabularyRegistry::global()?;
        let term = "oa";

        // Récupération explicite pour garantir la durée de vie
        let registry_context = registry.get_default_context();
        let r_iri = registry_context.get(term).unwrap();
        let m_iri = manager.active_mappings.get(term).unwrap();

        assert!(
            SharedRef::ptr_eq(r_iri, m_iri),
            "ÉCHEC INTERNING : Allocation mémoire redondante détectée."
        );

        Ok(())
    }

    /// 💎 TEST : Protection contre la récursion infinie (Cycles).
    #[async_test]
    #[serial_test::serial]
    async fn test_infinite_recursion_protection() -> RaiseResult<()> {
        crate::utils::testing::mock::inject_mock_config().await; // 🎯 FIX

        let mut manager = ContextManager::new()?;
        // Création d'un cycle vicieux
        manager
            .active_mappings
            .insert("A".to_string(), SharedRef::from("B"));
        manager
            .active_mappings
            .insert("B".to_string(), SharedRef::from("A"));

        let result = manager.expand_term("A");
        assert_eq!(result, "A"); // Doit s'arrêter sur le terme stable

        Ok(())
    }

    /// 💎 TEST : Résolution d'alias d'alias (Recursion profonde).
    #[async_test]
    #[serial_test::serial]
    async fn test_deep_alias_resolution() -> RaiseResult<()> {
        crate::utils::testing::mock::inject_mock_config().await; // 🎯 FIX

        let mut manager = ContextManager::new()?;
        // Alias chain : Acteur -> oa:OperationalActor -> https://raise.io/oa#OperationalActor
        let doc = json_value!({ "@context": { "Acteur": "oa:OperationalActor" } });
        manager.load_from_doc(&doc)?;

        let expanded = manager.expand_term("Acteur");
        assert_eq!(expanded, "https://raise.io/oa#OperationalActor");

        Ok(())
    }

    /// 💎 TEST : Idempotence et Cycle de vie (Expand <-> Compact).
    #[async_test]
    #[serial_test::serial]
    async fn test_expansion_compaction_roundtrip() -> RaiseResult<()> {
        crate::utils::testing::mock::inject_mock_config().await; // 🎯 FIX

        let manager = ContextManager::new()?;
        let cases = vec!["oa:Activity", "rdfs:label"];
        for term in cases {
            let expanded = manager.expand_term(term);
            assert_eq!(term, manager.compact_iri(&expanded));
        }

        Ok(())
    }

    /// 💎 TEST : Résilience face aux injections JSON invalides.
    #[async_test]
    #[serial_test::serial]
    async fn test_load_malformed_json_resilience() -> RaiseResult<()> {
        crate::utils::testing::mock::inject_mock_config().await;

        let mut manager = ContextManager::new()?;
        let count_before = manager.active_mappings.len();
        let _ = manager.load_from_doc(&json_value!(null));
        let _ = manager.load_from_doc(&json_value!({"@context": [123]}));
        assert_eq!(manager.active_mappings.len(), count_before);

        Ok(())
    }
}
