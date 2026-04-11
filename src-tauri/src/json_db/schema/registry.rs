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
            base_prefix: "db://unknown/unknown/schemas/v2/".to_string(),
            schemas_root: None,
        }
    }

    pub async fn from_db(config: &JsonDbConfig, space: &str, db: &str) -> RaiseResult<Self> {
        let base_prefix = format!("db://{}/{}/schemas/", space, db);
        let schemas_root = config.db_schemas_root(space, db);

        let mut registry = Self {
            by_uri: UnorderedMap::new(),
            base_prefix: base_prefix.clone(),
            schemas_root: Some(schemas_root.clone()),
        };

        let app_config = AppConfig::get();

        // 🎯 FIX : Chargement en cascade via les Points de Montage stricts

        // 1. Noyau Système
        registry
            .load_domain_schemas(
                config,
                &app_config.mount_points.system.domain,
                &app_config.mount_points.system.db,
            )
            .await?;

        // 2. Raise Core
        registry
            .load_domain_schemas(
                config,
                &app_config.mount_points.raise.domain,
                &app_config.mount_points.raise.db,
            )
            .await?;

        // 3. Workspace MBSE
        registry
            .load_domain_schemas(
                config,
                &app_config.mount_points.simulation.domain,
                &app_config.mount_points.simulation.db,
            )
            .await?;

        // 4. Domaine courant (si différent)
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
        let root = config.db_schemas_root(space, db);

        if fs::exists_async(&root).await {
            let mut entries = match fs::read_dir_async(&root).await {
                Ok(e) => e,
                Err(e) => raise_error!(
                    "ERR_FS_READ_DIR",
                    error = e,
                    context = json_value!({ "path": root, "action": "load_domain_schemas" })
                ),
            };

            while let Some(entry) = match entries.next_entry().await {
                Ok(e) => e,
                Err(e) => raise_error!(
                    "ERR_FS_SCAN_NEXT_ENTRY",
                    error = e,
                    context = json_value!({"path": root})
                ),
            } {
                let path = entry.path();
                let file_type = match entry.file_type().await {
                    Ok(ft) => ft,
                    Err(e) => raise_error!(
                        "ERR_FS_GET_FILE_TYPE",
                        error = e,
                        context = json_value!({"path": path})
                    ),
                };

                if file_type.is_dir() {
                    let dir_name = entry.file_name().to_string_lossy().to_string();
                    if dir_name.starts_with('v') {
                        let prefix = format!("db://{}/{}/schemas/{}/", space, db, dir_name);
                        self.scan_directory(&path, &path, &prefix).await?;
                    }
                }
            }
        }
        Ok(())
    }

    #[async_recursive]
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
                self.scan_directory(root, &path, prefix).await?;
            } else if file_type.is_file() && path.extension().is_some_and(|e| e == "json") {
                if let Ok(content) = fs::read_to_string_async(&path).await {
                    if let Ok(schema) = json::deserialize_from_str(&content) {
                        if let Ok(rel_path) = path.strip_prefix(root) {
                            let rel_str = rel_path.to_string_lossy().replace('\\', "/");
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

        // 2. 🎯 FALLBACK INTELLIGENT (Virtualisation en mémoire vers les mount_points)
        if let Some(idx) = uri.find("/schemas/") {
            let remainder = &uri[idx + "/schemas/".len()..];
            let parts: Vec<&str> = remainder.splitn(2, '/').collect();

            if parts.len() == 2 {
                let version = parts[0];
                let relative_path = parts[1];
                let app_config = AppConfig::get();

                // A. Workspace MBSE
                let mod_uri = format!(
                    "db://{}/{}/schemas/{}/{}",
                    app_config.mount_points.modeling.domain,
                    app_config.mount_points.modeling.db,
                    version,
                    relative_path
                );
                if let Some(schema) = self.by_uri.get(&mod_uri) {
                    return Some(schema);
                }

                // B. Raise Core
                let raise_uri = format!(
                    "db://{}/{}/schemas/{}/{}",
                    app_config.mount_points.raise.domain,
                    app_config.mount_points.raise.db,
                    version,
                    relative_path
                );
                if let Some(schema) = self.by_uri.get(&raise_uri) {
                    return Some(schema);
                }

                // C. Noyau Système Configuré
                let sys_uri = format!(
                    "db://{}/{}/schemas/{}/{}",
                    app_config.mount_points.system.domain,
                    app_config.mount_points.system.db,
                    version,
                    relative_path
                );
                if let Some(schema) = self.by_uri.get(&sys_uri) {
                    return Some(schema);
                }

                // D. _system/_system en ultime recours
                let hard_sys_uri =
                    format!("db://_system/_system/schemas/{}/{}", version, relative_path);
                if let Some(schema) = self.by_uri.get(&hard_sys_uri) {
                    return Some(schema);
                }
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
        // 🎯 FIX : Retrait du "return" devant raise_error!
        let Some(root) = self.schemas_root.as_ref() else {
            raise_error!(
                "ERR_SCHEMA_NO_ROOT",
                error = "Opération impossible : le registre n'est pas lié à un espace disque."
            );
        };

        let Some(idx) = uri.find("/schemas/") else {
            raise_error!(
                "ERR_SCHEMA_URI_MISMATCH",
                error = format!("URI invalide : {}", uri)
            );
        };

        let rel_path = &uri[idx + "/schemas/".len()..];
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
        // 🎯 FIX : Retrait du "return" devant raise_error!
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
        // 🎯 FIX : Retrait du "return" devant raise_error!
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
        // 🎯 FIX : Retrait du "return" devant raise_error!
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
    use crate::utils::io::fs::tempdir;
    use crate::utils::testing::mock::inject_mock_config;

    #[async_test]
    async fn test_registry_loading_multi_versions() -> RaiseResult<()> {
        inject_mock_config().await;

        let dir = match tempdir() {
            Ok(d) => d,
            Err(e) => panic!("Erreur lors de la création du dossier temporaire : {:?}", e),
        };

        let config = JsonDbConfig::new(dir.path().to_path_buf());
        let _storage = StorageEngine::new(config.clone());

        let space = "s1";
        let db = "d1";

        // Création structure v1
        let schema_dir_v1 = config.db_schemas_root(space, db).join("v1/users");
        if let Err(e) = fs::ensure_dir_async(&schema_dir_v1).await {
            panic!("Échec création dossier v1 : {:?}", e);
        }

        if let Err(e) = fs::write_json_atomic_async(
            &schema_dir_v1.join("user.schema.json"),
            &json_value!({ "type": "object" }),
        )
        .await
        {
            panic!("Échec de l'écriture du schéma v1 : {:?}", e);
        }

        // Création structure v2
        let schema_dir_v2 = config.db_schemas_root(space, db).join("v2/users");
        if let Err(e) = fs::ensure_dir_async(&schema_dir_v2).await {
            panic!("Échec création dossier v2 : {:?}", e);
        }
        if let Err(e) = fs::write_json_atomic_async(
            &schema_dir_v2.join("user.schema.json"),
            &json_value!({ "type": "object" }),
        )
        .await
        {
            panic!("Échec de l'écriture du schéma v2 : {:?}", e);
        }

        let reg = SchemaRegistry::from_db(&config, space, db).await?;

        // Vérification que les deux versions cohabitent
        assert!(reg
            .get_by_uri(&format!(
                "db://{}/{}/schemas/v1/users/user.schema.json",
                space, db
            ))
            .is_some());

        assert!(reg
            .get_by_uri(&format!(
                "db://{}/{}/schemas/v2/users/user.schema.json",
                space, db
            ))
            .is_some());

        Ok(())
    }

    #[async_test]
    async fn test_schema_ddl_operations() -> RaiseResult<()> {
        inject_mock_config().await;

        let dir = match tempdir() {
            Ok(d) => d,
            Err(e) => panic!("Erreur lors de la création du dossier temporaire : {:?}", e),
        };
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
        let schema_after_add = match reg.get_by_uri(&uri) {
            Some(s) => s,
            None => panic!("Le schéma devrait exister après ajout de propriété"),
        };
        assert!(schema_after_add["properties"]["price"].is_object());

        // 3. ALTER PROPERTY
        reg.alter_property(
            &uri,
            "price",
            json_value!({ "type": "number", "minimum": 0 }),
        )
        .await?;

        let schema_after_alter = match reg.get_by_uri(&uri) {
            Some(s) => s,
            None => panic!("Le schéma devrait exister après altération de propriété"),
        };
        assert_eq!(schema_after_alter["properties"]["price"]["minimum"], 0);

        // 4. DROP PROPERTY
        reg.drop_property(&uri, "name").await?;

        let schema_after_drop_prop = match reg.get_by_uri(&uri) {
            Some(s) => s,
            None => panic!("Le schéma devrait exister après suppression de propriété"),
        };
        let has_name = match schema_after_drop_prop
            .get("properties")
            .and_then(|p| p.as_object())
        {
            Some(obj) => obj.contains_key("name"),
            None => false,
        };
        assert!(!has_name, "La propriété 'name' devrait avoir disparu");

        // 5. DROP SCHEMA
        reg.drop_schema(&uri).await?;
        assert!(reg.get_by_uri(&uri).is_none());

        let path = match reg.get_physical_path(&uri) {
            Ok(p) => p,
            Err(e) => panic!("Impossible de résoudre le chemin physique : {:?}", e),
        };
        assert!(!fs::exists_async(&path).await);

        Ok(())
    }
}
