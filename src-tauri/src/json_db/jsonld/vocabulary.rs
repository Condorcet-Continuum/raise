// FICHIER : src-tauri/src/json_db/jsonld/vocabulary.rs

use crate::utils::prelude::*;
use std::future::Future;
use std::pin::Pin;

// --- STRUCTURES ---
#[derive(Debug, Clone, Serializable, Deserializable, PartialEq)]
pub enum PropertyType {
    DatatypeProperty,
    ObjectProperty,
}

#[derive(Debug, Clone, Serializable, Deserializable)]
pub struct Class {
    pub iri: String,
    pub label: String,
    pub comment: String,
    pub sub_class_of: Option<String>,
}

#[derive(Debug, Clone, Serializable, Deserializable)]
pub struct Property {
    pub iri: String,
    pub label: String,
    pub property_type: PropertyType,
    pub domain: Option<String>,
    pub range: Option<String>,
}

// --- REGISTRE PRINCIPAL (SINGLETON DYNAMIQUE) ---

static INSTANCE: StaticCell<VocabularyRegistry> = StaticCell::new();

pub struct VocabularyRegistry {
    classes: UnorderedMap<String, Class>,
    properties: UnorderedMap<String, Property>,
    default_context: UnorderedMap<String, String>,
    layer_contexts: SharedRef<SyncRwLock<UnorderedMap<String, JsonValue>>>,
}

impl Default for VocabularyRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl VocabularyRegistry {
    /// Crée une instance vide du registre (utile pour les tests isolés)
    pub fn new() -> Self {
        Self {
            classes: UnorderedMap::new(),
            properties: UnorderedMap::new(),
            default_context: UnorderedMap::new(),
            layer_contexts: SharedRef::new(SyncRwLock::new(UnorderedMap::new())),
        }
    }

    /// Initialise le registre global en scannant récursivement le dossier des ontologies.
    pub async fn init(ontology_root: &Path) -> RaiseResult<()> {
        let mut registry = Self {
            classes: UnorderedMap::new(),
            properties: UnorderedMap::new(),
            default_context: UnorderedMap::new(),
            layer_contexts: SharedRef::new(SyncRwLock::new(UnorderedMap::new())),
        };

        // Scan et chargement de tous les fichiers .jsonld
        registry.load_all_ontologies(ontology_root).await?;

        if INSTANCE.set(registry).is_err() {
            tracing::warn!("Le VocabularyRegistry a déjà été initialisé.");
        }

        Ok(())
    }

    /// Récupère l'instance globale. Panique si appelée avant `init()`.
    pub fn global() -> &'static Self {
        #[cfg(not(test))]
        {
            INSTANCE.get().expect("❌ VocabularyRegistry non initialisé ! Appelez VocabularyRegistry::init(path) au démarrage.")
        }

        #[cfg(test)]
        {
            // Auto-initialisation de secours isolée pour les tests unitaires
            if INSTANCE.get().is_none() {
                let mut registry = Self::new();

                // 1. Mappings vitaux pour la résolution d'URIs (ArcadiaOntology::get_uri)
                registry.default_context.insert(
                    "oa".to_string(),
                    "https://raise.io/ontology/arcadia/oa#".to_string(),
                );
                registry.default_context.insert(
                    "sa".to_string(),
                    "https://raise.io/ontology/arcadia/sa#".to_string(),
                );
                registry.default_context.insert(
                    "la".to_string(),
                    "https://raise.io/ontology/arcadia/la#".to_string(),
                );
                registry.default_context.insert(
                    "pa".to_string(),
                    "https://raise.io/ontology/arcadia/pa#".to_string(),
                );
                registry.default_context.insert(
                    "epbs".to_string(),
                    "https://raise.io/ontology/arcadia/epbs#".to_string(),
                );
                registry.default_context.insert(
                    "data".to_string(),
                    "https://raise.io/ontology/arcadia/data#".to_string(),
                );
                registry.default_context.insert(
                    "transverse".to_string(),
                    "https://raise.io/ontology/arcadia/transverse#".to_string(),
                );

                // 2. Règle sémantique mockée pour le validateur (test_domain_violation)
                registry.properties.insert(
                    "https://raise.io/ontology/arcadia/oa#involvesActivity".to_string(),
                    Property {
                        iri: "https://raise.io/ontology/arcadia/oa#involvesActivity".to_string(),
                        label: "involves activity".to_string(),
                        property_type: PropertyType::ObjectProperty,
                        domain: Some(
                            "https://raise.io/ontology/arcadia/oa#OperationalCapability"
                                .to_string(),
                        ),
                        range: Some(
                            "https://raise.io/ontology/arcadia/oa#OperationalActivity".to_string(),
                        ),
                    },
                );

                let _ = INSTANCE.set(registry); // On ignore l'erreur si un autre thread l'a déjà fait
            }
            INSTANCE.get().unwrap()
        }
    }

    /// Parcours récursif du dossier ontology/ (Zero Hardcoding)
    pub fn load_all_ontologies<'a>(
        &'a mut self,
        root: &'a Path,
    ) -> Pin<Box<dyn Future<Output = RaiseResult<()>> + Send + 'a>> {
        Box::pin(async move {
            let mut entries = match fs::read_dir_async(root).await {
                Ok(e) => e,
                Err(_) => return Ok(()), // Ignore silencieusement si le dossier n'existe pas (ex: tests)
            };

            while let Some(entry) = entries.next_entry().await.unwrap_or(None) {
                let path = entry.path();
                let file_type = entry.file_type().await.unwrap();

                if file_type.is_dir() {
                    self.load_all_ontologies(&path).await?;
                } else if path.extension().is_some_and(|ext| ext == "jsonld") {
                    let layer = path.file_stem().unwrap().to_string_lossy();
                    self.load_layer_from_file(&layer, &path).await?;
                }
            }
            Ok(())
        })
    }

    /// Charge un fichier .jsonld et extrait dynamiquement sa sémantique
    pub async fn load_layer_from_file(&mut self, layer: &str, path: &Path) -> RaiseResult<()> {
        let content = match fs::read_to_string_async(path).await {
            Ok(c) => c,
            Err(e) => raise_error!(
                "ERR_IO_READ_FAIL",
                error = e,
                context = json_value!({ "path": path.to_string_lossy() })
            ),
        };

        let json: JsonValue = match json::deserialize_from_str(&content) {
            Ok(j) => j,
            Err(e) => raise_error!(
                "ERR_JSON_PARSE_FAIL",
                error = e,
                context = json_value!({ "path": path.to_string_lossy() })
            ),
        };

        // 1. Validation du schéma JSON-LD
        let Some(ctx) = json.get("@context") else {
            raise_error!(
                "ERR_JSONLD_CONTEXT_MISSING",
                error = "Champ '@context' manquant.",
                context = json_value!({ "path": path.to_string_lossy() })
            ); // 🎯 La macro fait un 'return' automatique ici, pas besoin de return Err(...)
        };

        // 2. Mise en cache du contexte brut
        {
            let mut cache = self.layer_contexts.write().unwrap();
            cache.insert(layer.to_string(), ctx.clone());
        }

        // 3. Extraction dynamique des préfixes (pour le contexte global par défaut)
        if let Some(ctx_obj) = ctx.as_object() {
            for (key, val) in ctx_obj {
                if let Some(iri) = val.as_str() {
                    self.default_context.insert(key.clone(), iri.to_string());
                } else if let Some(obj) = val.as_object() {
                    if let Some(id) = obj.get("@id").and_then(|v| v.as_str()) {
                        self.default_context.insert(key.clone(), id.to_string());
                    }
                }
            }
        }

        // 4. Extraction dynamique des Classes et Propriétés depuis le @graph
        if let Some(graph) = json.get("@graph").and_then(|v| v.as_array()) {
            for node in graph {
                if let Some(id) = node.get("@id").and_then(|v| v.as_str()) {
                    let types = extract_types(node);

                    // Détection des Classes
                    if types.contains(&"owl:Class".to_string())
                        || types.contains(&"rdfs:Class".to_string())
                    {
                        self.classes.insert(
                            id.to_string(),
                            Class {
                                iri: id.to_string(),
                                label: get_string_prop(node, "rdfs:label")
                                    .unwrap_or_else(|| id.to_string()),
                                comment: get_string_prop(node, "rdfs:comment").unwrap_or_default(),
                                sub_class_of: get_string_prop(node, "rdfs:subClassOf"),
                            },
                        );
                    }

                    // Détection des Propriétés (Object / Datatype)
                    let is_obj_prop = types.contains(&"owl:ObjectProperty".to_string());
                    let is_data_prop = types.contains(&"owl:DatatypeProperty".to_string());

                    if is_obj_prop || is_data_prop {
                        self.properties.insert(
                            id.to_string(),
                            Property {
                                iri: id.to_string(),
                                label: get_string_prop(node, "rdfs:label")
                                    .unwrap_or_else(|| id.to_string()),
                                property_type: if is_obj_prop {
                                    PropertyType::ObjectProperty
                                } else {
                                    PropertyType::DatatypeProperty
                                },
                                domain: get_string_prop(node, "rdfs:domain"),
                                range: get_string_prop(node, "rdfs:range"),
                            },
                        );
                    }
                }
            }
        }

        #[cfg(debug_assertions)]
        println!("✅ Ontologie dynamique chargée : {} -> {:?}", layer, path);

        Ok(())
    }

    pub fn get_context_for_layer(&self, layer: &str) -> Option<JsonValue> {
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

    pub fn get_default_context(&self) -> &UnorderedMap<String, String> {
        &self.default_context
    }

    pub fn is_iri(term: &str) -> bool {
        term.starts_with("http://") || term.starts_with("https://") || term.starts_with("urn:")
    }

    pub fn init_mock_for_tests() {
        if INSTANCE.get().is_none() {
            let mut registry = Self::new();

            // Mappings vitaux pour la résolution d'URIs IA
            registry.default_context.insert(
                "oa".to_string(),
                "https://raise.io/ontology/arcadia/oa#".to_string(),
            );
            registry.default_context.insert(
                "sa".to_string(),
                "https://raise.io/ontology/arcadia/sa#".to_string(),
            );
            registry.default_context.insert(
                "la".to_string(),
                "https://raise.io/ontology/arcadia/la#".to_string(),
            );
            registry.default_context.insert(
                "pa".to_string(),
                "https://raise.io/ontology/arcadia/pa#".to_string(),
            );
            registry.default_context.insert(
                "transverse".to_string(),
                "https://raise.io/ontology/arcadia/transverse#".to_string(),
            );

            let _ = INSTANCE.set(registry);
        }
    }
}

// --- UTILITAIRES DE PARSING JSON-LD ---

fn extract_types(node: &JsonValue) -> Vec<String> {
    let mut types = Vec::new();
    let extract = |val: &JsonValue, out: &mut Vec<String>| {
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
    if let Some(t) = node.get("@type") {
        extract(t, &mut types);
    }
    if let Some(t) = node.get("http://www.w3.org/1999/02/22-rdf-syntax-ns#type") {
        extract(t, &mut types);
    }
    types
}

fn get_string_prop(node: &JsonValue, key: &str) -> Option<String> {
    node.get(key).and_then(|v| {
        if let Some(s) = v.as_str() {
            Some(s.to_string())
        } else if let Some(obj) = v.as_object() {
            obj.get("@id")
                .or(obj.get("@value"))
                .and_then(|id| id.as_str().map(String::from))
        } else if let Some(arr) = v.as_array() {
            arr.first().and_then(|first| {
                if let Some(s) = first.as_str() {
                    Some(s.to_string())
                } else if let Some(obj) = first.as_object() {
                    obj.get("@id")
                        .or(obj.get("@value"))
                        .and_then(|id| id.as_str().map(String::from))
                } else {
                    None
                }
            })
        } else {
            None
        }
    })
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_iri() {
        assert!(VocabularyRegistry::is_iri("http://raise.io"));
        assert!(VocabularyRegistry::is_iri("urn:uuid:123"));
        assert!(!VocabularyRegistry::is_iri("oa:Activity"));
    }

    #[async_test]
    async fn test_dynamic_parsing_and_inheritance() {
        let mut reg = VocabularyRegistry {
            classes: UnorderedMap::new(),
            properties: UnorderedMap::new(),
            default_context: UnorderedMap::new(),
            layer_contexts: SharedRef::new(SyncRwLock::new(UnorderedMap::new())),
        };

        // Fichier JSON-LD simulé
        let mock_file_path = crate::utils::io::fs::PathBuf::from("/tmp/mock.jsonld");
        crate::utils::io::fs::write_json_atomic_async(
            &mock_file_path,
            &json_value!({
                "@context": {
                    "test": "http://test.org/"
                },
                "@graph": [
                    {
                        "@id": "http://test.org/Animal",
                        "@type": "owl:Class",
                        "rdfs:label": "Animal"
                    },
                    {
                        "@id": "http://test.org/Chat",
                        "@type": "owl:Class",
                        "rdfs:subClassOf": "http://test.org/Animal"
                    }
                ]
            }),
        )
        .await
        .unwrap();

        reg.load_layer_from_file("mock", &mock_file_path)
            .await
            .unwrap();

        // 1. Vérifie l'extraction du préfixe
        assert_eq!(
            reg.get_default_context().get("test").unwrap(),
            "http://test.org/"
        );

        // 2. Vérifie l'extraction des classes
        assert!(reg.has_class("http://test.org/Animal"));
        assert!(reg.has_class("http://test.org/Chat"));

        // 3. Vérifie l'héritage
        assert!(reg.is_subtype_of("http://test.org/Chat", "http://test.org/Animal"));
        assert!(!reg.is_subtype_of("http://test.org/Animal", "http://test.org/Chat"));
    }
}
