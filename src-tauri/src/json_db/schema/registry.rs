// FICHIER : src-tauri/src/json_db/schema/registry.rs

use crate::json_db::storage::JsonDbConfig;
use crate::utils::{
    async_recursion,
    error::AnyResult,
    fs::{self, Path},
    json::{self, Value},
    HashMap,
};

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

    pub async fn from_db(config: &JsonDbConfig, space: &str, db: &str) -> AnyResult<Self> {
        // Préfixe standard : db://space/db/schemas/v1/
        let base_prefix = format!("db://{}/{}/schemas/v1/", space, db);

        let mut registry = Self {
            by_uri: HashMap::new(),
            base_prefix: base_prefix.clone(),
        };

        let schemas_root = config.db_schemas_root(space, db).join("v1");

        if !fs::exists(&schemas_root).await {
            return Ok(registry);
        }
        registry
            .scan_directory(&schemas_root, &schemas_root)
            .await?;
        Ok(registry)
    }

    #[async_recursion]
    async fn scan_directory(&mut self, root: &Path, current_dir: &Path) -> AnyResult<()> {
        // Utilisation de read_dir de la façade
        let mut entries = fs::read_dir(current_dir).await?;

        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(crate::utils::error::AppError::Io)?
        {
            let path = entry.path();
            let file_type = entry
                .file_type()
                .await
                .map_err(crate::utils::error::AppError::Io)?;

            if file_type.is_dir() {
                // Récursion asynchrone via la macro de la façade
                self.scan_directory(root, &path).await?;
            } else if file_type.is_file() && path.extension().is_some_and(|e| e == "json") {
                // Lecture asynchrone via la façade
                if let Ok(content) = fs::read_to_string(&path).await {
                    if let Ok(schema) = json::parse(&content) {
                        if let Ok(rel_path) = path.strip_prefix(root) {
                            let rel_str = rel_path.to_string_lossy().replace("\\", "/");
                            let uri = format!("{}{}", self.base_prefix, rel_str);
                            self.register(uri, schema);
                        }
                    }
                }
            }
        }
        Ok(())
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
    use crate::utils::{fs::tempdir, json::json};

    #[tokio::test]
    async fn test_registry_loading() -> AnyResult<()> {
        // 1. Setup environnement
        let dir = tempdir().unwrap();
        let config = JsonDbConfig::new(dir.path().to_path_buf());
        let _storage = StorageEngine::new(config.clone()); // Juste pour init

        let space = "s1";
        let db = "d1";

        // Création structure : schemas/v1/users/user.schema.json
        let schema_dir = config.db_schemas_root(space, db).join("v1/users");
        fs::ensure_dir(&schema_dir)
            .await
            .expect("Échec création dossier");

        let schema_content = json!({
            "type": "object",
            "properties": { "name": { "type": "string" } }
        });
        fs::write_json_atomic(&schema_dir.join("user.schema.json"), &schema_content)
            .await
            .expect("Échec écriture schéma");

        // 2. Chargement
        let reg = SchemaRegistry::from_db(&config, space, db).await?;

        // 3. Vérification
        let expected_uri = format!("db://{}/{}/schemas/v1/users/user.schema.json", space, db);
        assert!(reg.get_by_uri(&expected_uri).is_some());
        assert_eq!(reg.uri("users/user.schema.json"), expected_uri);

        Ok(())
    }
}
