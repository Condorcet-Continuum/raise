// FICHIER : src-tauri/src/json_db/schema/registry.rs

use crate::json_db::storage::JsonDbConfig;
use crate::utils::prelude::*;

#[derive(Debug, Clone)]
pub struct SchemaRegistry {
    pub(crate) by_uri: UnorderedMap<String, JsonValue>,
    pub base_prefix: String,
    pub(crate) schemas_root: Option<PathBuf>,
}

impl Default for SchemaRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl SchemaRegistry {
    pub fn new() -> Self {
        Self {
            by_uri: UnorderedMap::new(),
            base_prefix: "db://unknown/unknown/schemas/v1/".to_string(),
            schemas_root: None,
        }
    }

    pub async fn from_db(config: &JsonDbConfig, space: &str, db: &str) -> RaiseResult<Self> {
        let base_prefix = format!("db://{}/{}/schemas/v1/", space, db);
        let schemas_root = config.db_schemas_root(space, db).join("v1");

        let mut registry = Self {
            by_uri: UnorderedMap::new(),
            base_prefix: base_prefix.clone(),
            schemas_root: Some(schemas_root.clone()),
        };

        let app_config = AppConfig::get();

        // 🎯 LECTURE ASCENDANTE : On charge toutes les strates dans la mémoire (UnorderedMap)
        // 1. Noyau système dur (_system/_system)
        registry
            .load_domain_schemas(config, "_system", "_system")
            .await?;

        // 2. Système configuré
        if app_config.system_domain != "_system" || app_config.system_db != "_system" {
            registry
                .load_domain_schemas(config, &app_config.system_domain, &app_config.system_db)
                .await?;
        }

        // 3. Workstation
        if let Some(ws) = &app_config.workstation {
            if let (Some(d), Some(b)) = (&ws.default_domain, &ws.default_db) {
                registry.load_domain_schemas(config, d, b).await?;
            }
        }

        // 4. Utilisateur
        if let Some(user) = &app_config.user {
            if let (Some(d), Some(b)) = (&user.default_domain, &user.default_db) {
                registry.load_domain_schemas(config, d, b).await?;
            }
        }

        // 5. Domaine courant (space/db)
        registry.load_domain_schemas(config, space, db).await?;

        Ok(registry)
    }

    /// Helper privé pour charger un domaine spécifique dans le registre
    async fn load_domain_schemas(
        &mut self,
        config: &JsonDbConfig,
        space: &str,
        db: &str,
    ) -> RaiseResult<()> {
        let prefix = format!("db://{}/{}/schemas/v1/", space, db);
        let root = config.db_schemas_root(space, db).join("v1");
        if fs::exists_async(&root).await {
            self.scan_directory(&root, &root, &prefix).await?;
        }
        Ok(())
    }

    #[async_recursive] // Adapte le chemin de la macro selon tes imports
    async fn scan_directory(
        &mut self,
        root: &Path,
        current_dir: &Path,
        prefix: &str,
    ) -> RaiseResult<()> {
        let mut entries = fs::read_dir_async(current_dir).await?;

        while let Some(entry) = match entries.next_entry().await {
            Ok(e) => e,
            Err(e) => raise_error!(
                "ERR_FS_SCAN_NEXT_ENTRY",
                error = e,
                context = json_value!({ "dir": current_dir, "action": "scan_directory_recursion" })
            ),
        } {
            let path = entry.path();
            let file_type = match entry.file_type().await {
                Ok(ft) => ft,
                Err(e) => raise_error!(
                    "ERR_FS_GET_FILE_TYPE",
                    error = e,
                    context = json_value!({ "path": path })
                ),
            };

            if file_type.is_dir() {
                // 🎯 On passe le préfixe à la récursion
                self.scan_directory(root, &path, prefix).await?;
            } else if file_type.is_file() && path.extension().is_some_and(|e| e == "json") {
                if let Ok(content) = fs::read_to_string_async(&path).await {
                    if let Ok(schema) = json::deserialize_from_str(&content) {
                        if let Ok(rel_path) = path.strip_prefix(root) {
                            let rel_str = rel_path.to_string_lossy().replace("\\", "/");
                            // 🎯 L'URI utilise le préfixe du contexte chargé, pas forcément celui de la base
                            let uri = format!("{}{}", prefix, rel_str);
                            self.register(uri, schema);
                        }
                    }
                }
            }
        }
        Ok(())
    }

    pub fn register(&mut self, uri: String, schema: JsonValue) {
        self.by_uri.insert(uri, schema);
    }

    pub fn get_by_uri(&self, uri: &str) -> Option<&JsonValue> {
        // 1. Recherche stricte (Priorité absolue)
        if let Some(schema) = self.by_uri.get(uri) {
            return Some(schema);
        }

        // 2. 🎯 FALLBACK INTELLIGENT (Virtualisation en mémoire)
        if let Some(idx) = uri.find("/schemas/v1/") {
            let relative_path = &uri[idx + "/schemas/v1/".len()..];
            let app_config = AppConfig::get();

            // A. Domaine Utilisateur
            if let Some(user) = &app_config.user {
                if let (Some(d), Some(b)) = (&user.default_domain, &user.default_db) {
                    let user_uri = format!("db://{}/{}/schemas/v1/{}", d, b, relative_path);
                    if let Some(schema) = self.by_uri.get(&user_uri) {
                        return Some(schema);
                    }
                }
            }

            // B. Domaine Workstation
            if let Some(ws) = &app_config.workstation {
                if let (Some(d), Some(b)) = (&ws.default_domain, &ws.default_db) {
                    let ws_uri = format!("db://{}/{}/schemas/v1/{}", d, b, relative_path);
                    if let Some(schema) = self.by_uri.get(&ws_uri) {
                        return Some(schema);
                    }
                }
            }

            // C. Domaine Système Configuré
            let sys_uri = format!(
                "db://{}/{}/schemas/v1/{}",
                app_config.system_domain, app_config.system_db, relative_path
            );
            if let Some(schema) = self.by_uri.get(&sys_uri) {
                return Some(schema);
            }

            // D. Noyau Dur (_system/_system)
            let hard_sys_uri = format!("db://_system/_system/schemas/v1/{}", relative_path);
            if let Some(schema) = self.by_uri.get(&hard_sys_uri) {
                return Some(schema);
            }
        }

        None
    }

    pub fn list_uris(&self) -> Vec<String> {
        self.by_uri.keys().cloned().collect()
    }

    pub fn uri(&self, relative_path: &str) -> String {
        format!("{}{}", self.base_prefix, relative_path)
    }
    // ============================================================================
    // OPÉRATIONS DDL (Réservées au module json_db)
    // ============================================================================

    /// Résout le chemin physique d'un schéma sur le disque
    fn get_physical_path(&self, uri: &str) -> RaiseResult<PathBuf> {
        // 🎯 Utilisation de let-else au lieu de ok_or_else
        let Some(root) = self.schemas_root.as_ref() else {
            raise_error!(
                "ERR_SCHEMA_NO_ROOT",
                error = "Opération impossible : le registre n'est pas lié à un espace disque."
            );
        };

        let Some(rel_path) = uri.strip_prefix(&self.base_prefix) else {
            raise_error!(
                "ERR_SCHEMA_URI_MISMATCH",
                error = format!("L'URI '{}' n'appartient pas à ce registre.", uri)
            );
        };

        Ok(root.join(rel_path))
    }

    /// Sauvegarde physique et en mémoire
    async fn save_schema(&mut self, uri: &str, schema: JsonValue) -> RaiseResult<()> {
        let path = self.get_physical_path(uri)?;
        if let Some(parent) = path.parent() {
            fs::ensure_dir_async(parent).await?;
        }
        fs::write_json_atomic_async(&path, &schema).await?;
        self.by_uri.insert(uri.to_string(), schema);
        Ok(())
    }

    // --- LES MÉTHODES PUBLIQUES RESTREINTES ---

    pub(in crate::json_db) async fn create_schema(
        &mut self,
        uri: &str,
        schema: JsonValue,
    ) -> RaiseResult<()> {
        if self.by_uri.contains_key(uri) {
            // 🎯 Appel direct (la macro fait le return Err toute seule)
            raise_error!(
                "ERR_SCHEMA_ALREADY_EXISTS",
                error = format!("Le schéma '{}' existe déjà.", uri)
            );
        }
        self.save_schema(uri, schema).await
    }

    pub(in crate::json_db) async fn drop_schema(&mut self, uri: &str) -> RaiseResult<()> {
        if !self.by_uri.contains_key(uri) {
            raise_error!("ERR_SCHEMA_NOT_FOUND", error = "Schéma introuvable.");
        }
        let path = self.get_physical_path(uri)?;
        if fs::exists_async(&path).await {
            fs::remove_file_async(&path).await?;
        }
        self.by_uri.remove(uri);
        Ok(())
    }

    pub(in crate::json_db) async fn add_property(
        &mut self,
        uri: &str,
        prop_name: &str,
        prop_def: JsonValue,
    ) -> RaiseResult<()> {
        let Some(mut schema) = self.by_uri.get(uri).cloned() else {
            raise_error!(
                "ERR_SCHEMA_NOT_FOUND",
                error = format!("Schéma introuvable : {}", uri)
            );
        };

        if schema.get("properties").is_none() {
            if let Some(obj) = schema.as_object_mut() {
                obj.insert("properties".to_string(), json_value!({}));
            }
        }

        if let Some(props) = schema.get_mut("properties").and_then(|v| v.as_object_mut()) {
            if props.contains_key(prop_name) {
                raise_error!(
                    "ERR_SCHEMA_PROP_ALREADY_EXISTS",
                    error = format!("La propriété '{}' existe déjà.", prop_name)
                );
            }
            props.insert(prop_name.to_string(), prop_def);
        }

        self.save_schema(uri, schema).await
    }

    pub(in crate::json_db) async fn alter_property(
        &mut self,
        uri: &str,
        prop_name: &str,
        prop_def: JsonValue,
    ) -> RaiseResult<()> {
        let Some(mut schema) = self.by_uri.get(uri).cloned() else {
            raise_error!("ERR_SCHEMA_NOT_FOUND", error = "Schéma introuvable.");
        };

        if let Some(props) = schema.get_mut("properties").and_then(|v| v.as_object_mut()) {
            if !props.contains_key(prop_name) {
                raise_error!(
                    "ERR_SCHEMA_PROP_NOT_FOUND",
                    error = format!("La propriété '{}' est introuvable.", prop_name)
                );
            }
            props.insert(prop_name.to_string(), prop_def);
        }

        self.save_schema(uri, schema).await
    }

    pub(in crate::json_db) async fn drop_property(
        &mut self,
        uri: &str,
        prop_name: &str,
    ) -> RaiseResult<()> {
        let Some(mut schema) = self.by_uri.get(uri).cloned() else {
            raise_error!("ERR_SCHEMA_NOT_FOUND", error = "Schéma introuvable.");
        };

        if let Some(props) = schema.get_mut("properties").and_then(|v| v.as_object_mut()) {
            if props.remove(prop_name).is_none() {
                raise_error!(
                    "ERR_SCHEMA_PROP_NOT_FOUND",
                    error = format!("La propriété '{}' est introuvable.", prop_name)
                );
            }
        }

        self.save_schema(uri, schema).await
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json_db::storage::{JsonDbConfig, StorageEngine};
    use crate::utils::testing::mock::inject_mock_config;

    #[async_test]
    async fn test_registry_loading() -> RaiseResult<()> {
        inject_mock_config().await;
        // 1. Setup environnement
        let dir = tempdir().unwrap();
        let config = JsonDbConfig::new(dir.path().to_path_buf());
        let _storage = StorageEngine::new(config.clone()); // Juste pour init

        let space = "s1";
        let db = "d1";

        // Création structure : schemas/v1/users/user.schema.json
        let schema_dir = config.db_schemas_root(space, db).join("v1/users");
        fs::ensure_dir_async(&schema_dir)
            .await
            .expect("Échec création dossier");

        let schema_content = json_value!({
            "type": "object",
            "properties": { "name": { "type": "string" } }
        });
        fs::write_json_atomic_async(&schema_dir.join("user.schema.json"), &schema_content)
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
    #[async_test]
    async fn test_schema_ddl_operations() -> RaiseResult<()> {
        inject_mock_config().await;

        let dir = tempdir().unwrap();
        let config = JsonDbConfig::new(dir.path().to_path_buf());
        let space = "s1";
        let db = "d1";

        let mut reg = SchemaRegistry::from_db(&config, space, db).await?;
        let uri = reg.uri("products/product.schema.json");

        // 1. CREATE SCHEMA
        let initial_schema = json_value!({
            "type": "object",
            "properties": { "name": { "type": "string" } }
        });
        reg.create_schema(&uri, initial_schema).await?;
        assert!(reg.get_by_uri(&uri).is_some());

        // 2. ADD PROPERTY
        reg.add_property(&uri, "price", json_value!({ "type": "number" }))
            .await?;
        let schema_after_add = reg.get_by_uri(&uri).unwrap();
        assert!(schema_after_add["properties"]["price"].is_object());

        // 3. ALTER PROPERTY
        reg.alter_property(
            &uri,
            "price",
            json_value!({ "type": "number", "minimum": 0 }),
        )
        .await?;
        let schema_after_alter = reg.get_by_uri(&uri).unwrap();
        assert_eq!(schema_after_alter["properties"]["price"]["minimum"], 0);

        // 4. DROP PROPERTY
        reg.drop_property(&uri, "name").await?;
        let schema_after_drop_prop = reg.get_by_uri(&uri).unwrap();
        assert!(schema_after_drop_prop["properties"]
            .as_object()
            .unwrap()
            .get("name")
            .is_none());

        // 5. DROP SCHEMA
        reg.drop_schema(&uri).await?;
        assert!(reg.get_by_uri(&uri).is_none());
        assert!(!fs::exists_async(&reg.get_physical_path(&uri).unwrap()).await);

        Ok(())
    }
}
