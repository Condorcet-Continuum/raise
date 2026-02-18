// FICHIER : src-tauri/src/traceability/tracer.rs

use crate::utils::HashMap;

use crate::model_engine::types::{ArcadiaElement, ProjectModel};

/// Service principal de traçabilité.
pub struct Tracer<'a> {
    model: &'a ProjectModel,
    // Index inversé : Target ID -> List of Source IDs
    reverse_links: HashMap<String, Vec<String>>,
}

impl<'a> Tracer<'a> {
    pub fn new(model: &'a ProjectModel) -> Self {
        let mut tracer = Self {
            model,
            reverse_links: HashMap::new(),
        };
        tracer.index_links();
        tracer
    }

    fn index_links(&mut self) {
        let elements = self.collect_all_elements();
        let mut reverse_map: HashMap<String, Vec<String>> = HashMap::new();

        for element in elements {
            for (key, value) in &element.properties {
                if is_link_property(key) {
                    if let Some(target_id) = value.as_str() {
                        reverse_map
                            .entry(target_id.to_string())
                            .or_default()
                            .push(element.id.clone());
                    } else if let Some(targets) = value.as_array() {
                        for t in targets {
                            if let Some(target_id) = t.as_str() {
                                reverse_map
                                    .entry(target_id.to_string())
                                    .or_default()
                                    .push(element.id.clone());
                            }
                        }
                    }
                }
            }
        }
        self.reverse_links = reverse_map;
    }

    pub fn get_downstream_elements(&self, element_id: &str) -> Vec<&ArcadiaElement> {
        let mut results = Vec::new();
        if let Some(element) = self.find_element(element_id) {
            for (key, value) in &element.properties {
                if is_link_property(key) {
                    if let Some(id) = value.as_str() {
                        if let Some(found) = self.find_element(id) {
                            results.push(found);
                        }
                    } else if let Some(ids) = value.as_array() {
                        for id_val in ids {
                            if let Some(id_str) = id_val.as_str() {
                                if let Some(found) = self.find_element(id_str) {
                                    results.push(found);
                                }
                            }
                        }
                    }
                }
            }
        }
        results
    }

    pub fn get_upstream_elements(&self, element_id: &str) -> Vec<&ArcadiaElement> {
        let mut results = Vec::new();
        if let Some(source_ids) = self.reverse_links.get(element_id) {
            for id in source_ids {
                if let Some(found) = self.find_element(id) {
                    results.push(found);
                }
            }
        }
        results
    }

    pub fn find_element(&self, id: &str) -> Option<&ArcadiaElement> {
        self.collect_all_elements().into_iter().find(|e| e.id == id)
    }

    fn collect_all_elements(&self) -> Vec<&ArcadiaElement> {
        let mut all = Vec::new();
        all.extend(&self.model.sa.functions);
        all.extend(&self.model.sa.components);
        all.extend(&self.model.la.functions);
        all.extend(&self.model.la.components);
        all.extend(&self.model.pa.functions);
        all.extend(&self.model.pa.components);
        all
    }
}

fn is_link_property(key: &str) -> bool {
    matches!(
        key,
        "allocatedTo" | "realizedBy" | "satisfiedBy" | "verifiedBy" | "model_id"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model_engine::types::NameType;
    use crate::utils::data::json;

    #[test]
    fn test_reverse_indexing_ai_model() {
        let mut model = ProjectModel::default();
        let mut props = HashMap::new();
        props.insert("model_id".to_string(), json!("ai_1"));

        let report = ArcadiaElement {
            id: "rep_1".into(),
            name: NameType::String("Report".into()),
            kind: "QualityReport".into(),
            // CORRECTION : Initialisation du champ description ajouté récemment
            description: None,
            properties: props,
        };
        model.pa.components.push(report);

        let tracer = Tracer::new(&model);
        let upstream = tracer.get_upstream_elements("ai_1");
        assert_eq!(upstream.len(), 1);
        assert_eq!(upstream[0].id, "rep_1");
    }
}
