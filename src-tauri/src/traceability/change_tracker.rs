use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Serialize, Deserialize)]
pub struct ChangeLog {
    pub element_id: String,
    pub changes: Vec<FieldChange>,
    pub timestamp: i64,
    pub author: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FieldChange {
    pub field: String,
    pub old_value: Option<String>,
    pub new_value: Option<String>,
}

pub struct ChangeTracker;

impl Default for ChangeTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl ChangeTracker {
    pub fn new() -> Self {
        Self
    }

    /// Compare deux versions JSON d'un élément et détecte les modifications.
    pub fn diff(&self, id: &str, old: &Value, new: &Value) -> ChangeLog {
        let mut changes = Vec::new();

        if let (Some(old_obj), Some(new_obj)) = (old.as_object(), new.as_object()) {
            // Champs modifiés ou ajoutés
            for (key, new_val) in new_obj {
                let old_val = old_obj.get(key);
                if old_val != Some(new_val) {
                    changes.push(FieldChange {
                        field: key.clone(),
                        old_value: old_val.map(|v| v.to_string()),
                        new_value: Some(new_val.to_string()),
                    });
                }
            }

            // Champs supprimés
            for (key, old_val) in old_obj {
                if !new_obj.contains_key(key) {
                    changes.push(FieldChange {
                        field: key.clone(),
                        old_value: Some(old_val.to_string()),
                        new_value: None,
                    });
                }
            }
        }

        ChangeLog {
            element_id: id.to_string(),
            changes,
            timestamp: chrono::Utc::now().timestamp(),
            author: "System".to_string(), // À connecter avec l'auth
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_diff_detection() {
        let tracker = ChangeTracker::new();
        let id = "elem_123";

        let old_ver = json!({
            "name": "Old Name",
            "status": "Draft",
            "description": "To be removed"
        });

        let new_ver = json!({
            "name": "New Name",     // Modifié
            "status": "Draft",      // Inchangé
            "priority": "High"      // Ajouté
                                    // "description" est supprimé
        });

        let log = tracker.diff(id, &old_ver, &new_ver);

        assert_eq!(log.element_id, id);
        assert_eq!(log.changes.len(), 3); // name change, description removed, priority added

        // Vérification modification
        let name_change = log.changes.iter().find(|c| c.field == "name").unwrap();
        assert_eq!(name_change.old_value.as_deref(), Some("\"Old Name\""));
        assert_eq!(name_change.new_value.as_deref(), Some("\"New Name\""));

        // Vérification suppression
        let desc_change = log
            .changes
            .iter()
            .find(|c| c.field == "description")
            .unwrap();
        assert!(desc_change.new_value.is_none());

        // Vérification ajout
        let prio_change = log.changes.iter().find(|c| c.field == "priority").unwrap();
        assert!(prio_change.old_value.is_none());
    }
}
