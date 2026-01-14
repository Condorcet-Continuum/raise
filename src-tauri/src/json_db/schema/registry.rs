// FICHIER : src-tauri/src/json_db/schema/registry.rs

use crate::json_db::storage::JsonDbConfig;
use anyhow::Result;
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use walkdir::WalkDir;

#[derive(Debug, Clone)]
pub struct SchemaRegistry {
    pub(crate) by_uri: HashMap<String, Value>,
    // AJOUT : On stocke le préfixe de base pour pouvoir reconstruire des URIs
    pub base_prefix: String,
}

impl Default for SchemaRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl SchemaRegistry {
    pub fn new() -> Self {
        Self {
            by_uri: HashMap::new(),
            base_prefix: "db://unknown/unknown/schemas/v1/".to_string(),
        }
    }

    pub fn from_db(config: &JsonDbConfig, space: &str, db: &str) -> Result<Self> {
        // Préfixe standard : db://space/db/schemas/v1/
        let base_prefix = format!("db://{}/{}/schemas/v1/", space, db);

        let mut registry = Self {
            by_uri: HashMap::new(),
            base_prefix: base_prefix.clone(),
        };

        let schemas_root = config.db_schemas_root(space, db).join("v1");

        if !schemas_root.exists() {
            return Ok(registry);
        }

        for entry in WalkDir::new(&schemas_root)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            if path.is_file() && path.extension().is_some_and(|e| e == "json") {
                if let Ok(content) = fs::read_to_string(path) {
                    if let Ok(schema) = serde_json::from_str::<Value>(&content) {
                        if let Ok(rel_path) = path.strip_prefix(&schemas_root) {
                            let rel_str = rel_path.to_string_lossy().replace("\\", "/");
                            let uri = format!("{}{}", base_prefix, rel_str);
                            registry.register(uri, schema);
                        }
                    }
                }
            }
        }

        Ok(registry)
    }

    pub fn register(&mut self, uri: String, schema: Value) {
        self.by_uri.insert(uri, schema);
    }

    pub fn get_by_uri(&self, uri: &str) -> Option<&Value> {
        self.by_uri.get(uri)
    }

    pub fn list_uris(&self) -> Vec<String> {
        self.by_uri.keys().cloned().collect()
    }

    // AJOUT : La méthode manquante demandée par le compilateur
    pub fn uri(&self, relative_path: &str) -> String {
        format!("{}{}", self.base_prefix, relative_path)
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json_db::storage::{JsonDbConfig, StorageEngine};
    use serde_json::json;
    use tempfile::tempdir;

    #[test]
    fn test_registry_loading() {
        // 1. Setup environnement
        let dir = tempdir().unwrap();
        let config = JsonDbConfig::new(dir.path().to_path_buf());
        let _storage = StorageEngine::new(config.clone()); // Juste pour init

        let space = "s1";
        let db = "d1";

        // Création structure : schemas/v1/users/user.schema.json
        let schema_dir = config.db_schemas_root(space, db).join("v1/users");
        fs::create_dir_all(&schema_dir).unwrap();

        let schema_content = json!({
            "type": "object",
            "properties": { "name": { "type": "string" } }
        });
        fs::write(
            schema_dir.join("user.schema.json"),
            serde_json::to_string(&schema_content).unwrap(),
        )
        .unwrap();

        // 2. Chargement
        let registry = SchemaRegistry::from_db(&config, space, db).unwrap();

        // 3. Vérification
        let expected_uri = format!("db://{}/{}/schemas/v1/users/user.schema.json", space, db);
        assert!(registry.get_by_uri(&expected_uri).is_some());

        // Test helper uri()
        assert_eq!(registry.uri("users/user.schema.json"), expected_uri);
    }
}
