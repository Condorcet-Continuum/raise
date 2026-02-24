// FICHIER : src-tauri/src/traceability/tracer.rs

use crate::json_db::collections::manager::CollectionsManager;
use crate::json_db::jsonld::vocabulary::PropertyType;
use crate::json_db::jsonld::{ContextManager, VocabularyRegistry};
use crate::model_engine::types::ProjectModel;
use crate::utils::{prelude::*, HashMap};

/// Service principal de traçabilité basé sur un Graphe d'IDs.
/// 🎯 OPTIMISATION : Plus de durée de vie 'a, le Traceur possède son propre graphe orienté.
pub struct Tracer {
    // Graphe orienté : Source ID -> List of Target IDs (Downstream)
    downstream_links: HashMap<String, Vec<String>>,
    // Index inversé : Target ID -> List of Source IDs (Upstream)
    upstream_links: HashMap<String, Vec<String>>,
}

impl Tracer {
    /// 1. Initialisation depuis le nouveau JsonDb (Architecture Cible SSOT)
    pub async fn from_db(manager: &CollectionsManager<'_>) -> RaiseResult<Self> {
        let mut docs = Vec::new();
        if let Ok(collections) = manager.list_collections().await {
            for col in collections {
                if let Ok(col_docs) = manager.list_all(&col).await {
                    docs.extend(col_docs);
                }
            }
        }
        Ok(Self::build_graph(docs))
    }

    /// 2. Rétro-compatibilité : Initialisation depuis l'ancien ProjectModel
    pub fn from_legacy_model(model: &ProjectModel) -> Self {
        let mut docs = Vec::new();

        let mut collect = |elements: &Vec<crate::model_engine::types::ArcadiaElement>| {
            for e in elements {
                if let Ok(val) = crate::utils::data::to_value(e) {
                    docs.push(val);
                }
            }
        };

        collect(&model.sa.functions);
        collect(&model.sa.components);
        collect(&model.la.functions);
        collect(&model.la.components);
        collect(&model.pa.functions);
        collect(&model.pa.components);

        Self::build_graph(docs)
    }
    pub fn from_json_list(documents: Vec<Value>) -> Self {
        Self::build_graph(documents)
    }
    /// Construit le graphe d'adjacence à partir de n'importe quel document JSON
    fn build_graph(documents: Vec<Value>) -> Self {
        let mut downstream: HashMap<String, Vec<String>> = HashMap::new();
        let mut upstream: HashMap<String, Vec<String>> = HashMap::new();

        let ctx = ContextManager::new();
        let registry = VocabularyRegistry::global();

        for doc in documents {
            let id = match doc.get("id").and_then(|v| v.as_str()) {
                Some(id) => id.to_string(),
                None => continue,
            };

            // On supporte le format Legacy (sous-objet "properties") et le format JsonDb pur (racine)
            let properties_obj = doc
                .get("properties")
                .and_then(|p| p.as_object())
                .or_else(|| doc.as_object());

            if let Some(props) = properties_obj {
                for (key, value) in props {
                    if is_link_property(key, &ctx, registry) {
                        let mut targets = Vec::new();

                        if let Some(target_id) = value.as_str() {
                            targets.push(target_id.to_string());
                        } else if let Some(arr) = value.as_array() {
                            for t in arr {
                                if let Some(target_id) = t.as_str() {
                                    targets.push(target_id.to_string());
                                }
                            }
                        }

                        // Indexation croisée
                        for target_id in &targets {
                            upstream
                                .entry(target_id.clone())
                                .or_default()
                                .push(id.clone());
                        }

                        downstream.entry(id.clone()).or_default().extend(targets);
                    }
                }
            }
        }

        Self {
            downstream_links: downstream,
            upstream_links: upstream,
        }
    }

    pub fn get_downstream_ids(&self, element_id: &str) -> Vec<String> {
        self.downstream_links
            .get(element_id)
            .cloned()
            .unwrap_or_default()
    }

    pub fn get_upstream_ids(&self, element_id: &str) -> Vec<String> {
        self.upstream_links
            .get(element_id)
            .cloned()
            .unwrap_or_default()
    }
}

/// 🎯 L'INTELLIGENCE SÉMANTIQUE (JSON-LD)
fn is_link_property(key: &str, ctx: &ContextManager, registry: &VocabularyRegistry) -> bool {
    // 1. Compatibilité Legacy (Hardcoded Strings)
    if matches!(
        key,
        "allocatedTo" | "realizedBy" | "satisfiedBy" | "verifiedBy" | "model_id"
    ) {
        return true;
    }

    // 2. Résolution Sémantique : Est-ce une ObjectProperty dans l'ontologie RAISE ?
    let expanded_uri = ctx.expand_term(key);
    if let Some(prop) = registry.get_property(&expanded_uri) {
        return prop.property_type == PropertyType::ObjectProperty;
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model_engine::types::{ArcadiaElement, NameType};
    use crate::utils::data::json;

    #[test]
    fn test_reverse_indexing_ai_model() {
        let mut model = ProjectModel::default();
        let mut props = crate::utils::HashMap::new();
        props.insert("model_id".to_string(), json!("ai_1"));

        let report = ArcadiaElement {
            id: "rep_1".into(),
            name: NameType::String("Report".into()),
            kind: "QualityReport".into(),
            description: None,
            properties: props,
        };
        model.pa.components.push(report);

        let tracer = Tracer::from_legacy_model(&model);

        let upstream = tracer.get_upstream_ids("ai_1");
        assert_eq!(upstream.len(), 1);
        assert_eq!(upstream[0], "rep_1");
    }
}
