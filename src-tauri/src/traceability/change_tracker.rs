// FICHIER : src-tauri/src/traceability/change_tracker.rs

use crate::utils::prelude::*;

#[derive(Debug, Serialize, Deserialize)]
pub struct ChangeLog {
    pub element_id: String,
    pub changes: Vec<FieldChange>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct FieldChange {
    pub field: String,
    pub old_value: Option<String>,
    pub new_value: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ChangeTracker;

impl ChangeTracker {
    pub fn new() -> Self {
        Self
    }

    pub fn diff(&self, id: &str, old: &Value, new: &Value) -> ChangeLog {
        let mut changes = Vec::new();
        // Lancement de la comparaison récursive à la racine
        self.compare_recursive("", old, new, &mut changes);

        ChangeLog {
            element_id: id.to_string(),
            changes,
        }
    }

    /// Fonction de parcours profond (Deep Diff)
    fn compare_recursive(
        &self,
        current_path: &str,
        old: &Value,
        new: &Value,
        changes: &mut Vec<FieldChange>,
    ) {
        // Si les valeurs sont strictement identiques, on s'arrête
        if old == new {
            return;
        }

        match (old, new) {
            // Si on compare deux objets JSON, on entre à l'intérieur
            (Value::Object(old_map), Value::Object(new_map)) => {
                // 1. On vérifie les clés du nouveau (Modifiées ou Ajoutées)
                for (k, new_val) in new_map {
                    let next_path = if current_path.is_empty() {
                        k.clone()
                    } else {
                        format!("{}.{}", current_path, k)
                    };

                    if let Some(old_val) = old_map.get(k) {
                        self.compare_recursive(&next_path, old_val, new_val, changes);
                    } else {
                        // La clé n'existait pas avant : c'est un ajout
                        changes.push(FieldChange {
                            field: next_path,
                            old_value: None,
                            new_value: Some(new_val.to_string()),
                        });
                    }
                }

                // 2. On vérifie les clés de l'ancien (Supprimées)
                for (k, old_val) in old_map {
                    if !new_map.contains_key(k) {
                        let next_path = if current_path.is_empty() {
                            k.clone()
                        } else {
                            format!("{}.{}", current_path, k)
                        };
                        changes.push(FieldChange {
                            field: next_path,
                            old_value: Some(old_val.to_string()),
                            new_value: None,
                        });
                    }
                }
            }
            // Pour tous les autres types (Tableaux, String, Nombres, ou types différents), on logge la différence
            _ => {
                let path = if current_path.is_empty() {
                    "root".to_string()
                } else {
                    current_path.to_string()
                };
                changes.push(FieldChange {
                    field: path,
                    old_value: Some(old.to_string()),
                    new_value: Some(new.to_string()),
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::data::json;

    #[test]
    fn test_diff_logic_deep() {
        let tracker = ChangeTracker::new();

        let old = json!({
            "status": "Draft",
            "properties": {
                "capacity": 50,
                "color": "red"
            }
        });

        let new = json!({
            "status": "Final",
            "properties": {
                "capacity": 51,
                "weight": 100
            }
        });

        let res = tracker.diff("element_1", &old, &new);

        // On s'attend à 4 changements exacts :
        assert_eq!(res.changes.len(), 4);

        // 1. Modification simple
        let status_change = res.changes.iter().find(|c| c.field == "status").unwrap();
        assert_eq!(status_change.old_value.as_deref(), Some("\"Draft\""));
        assert_eq!(status_change.new_value.as_deref(), Some("\"Final\""));

        // 2. Modification profonde
        let cap_change = res
            .changes
            .iter()
            .find(|c| c.field == "properties.capacity")
            .unwrap();
        assert_eq!(cap_change.old_value.as_deref(), Some("50"));
        assert_eq!(cap_change.new_value.as_deref(), Some("51"));

        // 3. Suppression profonde
        let color_change = res
            .changes
            .iter()
            .find(|c| c.field == "properties.color")
            .unwrap();
        assert_eq!(color_change.new_value, None);

        // 4. Ajout profond
        let weight_change = res
            .changes
            .iter()
            .find(|c| c.field == "properties.weight")
            .unwrap();
        assert_eq!(weight_change.old_value, None);
    }
}
