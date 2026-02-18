use crate::utils::prelude::*;

#[derive(Debug, Serialize, Deserialize)]
pub struct ChangeLog {
    pub element_id: String,
    pub changes: Vec<FieldChange>,
}

#[derive(Debug, Serialize, Deserialize)]
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
        if let (Some(o), Some(n)) = (old.as_object(), new.as_object()) {
            for (k, nv) in n {
                let ov = o.get(k);
                if ov != Some(nv) {
                    changes.push(FieldChange {
                        field: k.clone(),
                        old_value: ov.map(|v| v.to_string()),
                        new_value: Some(nv.to_string()),
                    });
                }
            }
        }
        ChangeLog {
            element_id: id.to_string(),
            changes,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_diff_logic() {
        let tracker = ChangeTracker::new();
        let old = json!({"status": "Draft"});
        let new = json!({"status": "Final"});
        let res = tracker.diff("1", &old, &new);
        assert_eq!(res.changes[0].field, "status");
    }
}
