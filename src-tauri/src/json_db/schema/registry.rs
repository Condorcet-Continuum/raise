// FICHIER : src-tauri/src/json_db/schema/registry.rs

use crate::json_db::storage::JsonDbConfig;
use crate::utils::prelude::*;

#[derive(Debug, Clone)]
pub struct SchemaRegistry {
    pub(crate) by_uri: UnorderedMap<String, JsonValue>,
    pub base_prefix: String,
    // 🎯 Disparition de `schemas_root` ! Le registre n'a plus besoin de connaître l'arborescence physique.
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
        }
    }

    pub async fn from_db(config: &JsonDbConfig, space: &str, db: &str) -> RaiseResult<Self> {
        let base_prefix = format!("db://{}/{}/schemas/", space, db);

        let mut registry = Self {
            by_uri: UnorderedMap::new(),
            base_prefix,
        };

        let app_config = AppConfig::get();

        // 🎯 Chargement en cascade via les Points de Montage stricts
        // Le registre agrège en mémoire tous les catalogues DDL des _system.json

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

    /// 🎯 MOTEUR DE LECTURE : Lit l'index pour trouver les chemins, puis charge les fichiers physiques
    async fn load_domain_schemas(
        &mut self,
        config: &JsonDbConfig,
        space: &str,
        db: &str,
    ) -> RaiseResult<()> {
        use crate::json_db::storage::file_storage;
        use crate::utils::io::fs;

        if let Ok(Some(sys_doc)) = file_storage::read_system_index(config, space, db).await {
            // Lecture de l'annuaire DDL
            if let Some(schemas) = sys_doc.get("schemas").and_then(|s| s.as_object()) {
                for (version, v_obj) in schemas {
                    if let Some(obj) = v_obj.as_object() {
                        // _meta_ptr correspond au petit objet {"file": "..."} stocké dans l'index
                        for (rel_path, _meta_ptr) in obj {
                            let uri =
                                format!("db://{}/{}/schemas/{}/{}", space, db, version, rel_path);
                            let schema_path = config
                                .db_schemas_root(space, db)
                                .join(version)
                                .join(rel_path);

                            // 🎯 Lecture du fichier PHYSIQUE sur le disque !
                            if let Ok(schema_json) = fs::read_json_async(&schema_path).await {
                                self.register(uri, schema_json);
                            }
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

    pub async fn from_uri(
        config: &JsonDbConfig,
        uri: &str,
        fallback_space: &str,
        fallback_db: &str,
    ) -> RaiseResult<Self> {
        let mut target_space = fallback_space.to_string();
        let mut target_db = fallback_db.to_string();

        if let Some(without_scheme) = uri.strip_prefix("db://") {
            let parts: Vec<&str> = without_scheme.splitn(3, '/').collect();
            if parts.len() >= 2 {
                target_space = parts[0].to_string();
                target_db = parts[1].to_string();
            }
        }

        Self::from_db(config, &target_space, &target_db).await
    }

    pub fn get_by_uri(&self, uri: &str) -> Option<&JsonValue> {
        // 1. Recherche stricte
        if let Some(schema) = self.by_uri.get(uri) {
            return Some(schema);
        }

        // 2. Fallback intelligent (inchangé)
        if let Some(idx) = uri.find("/schemas/") {
            let remainder = &uri[idx + "/schemas/".len()..];
            let parts: Vec<&str> = remainder.splitn(2, '/').collect();

            if parts.len() == 2 {
                let version = parts[0];
                let relative_path = parts[1];
                let app_config = AppConfig::get();

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
    // OPÉRATIONS DDL EN MÉMOIRE (La sauvegarde physique est désormais gérée par CollectionsManager)
    // ============================================================================
    #[allow(dead_code)]
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
        self.by_uri.insert(uri.to_string(), schema);
        Ok(())
    }

    #[allow(dead_code)]
    pub(in crate::json_db) async fn drop_schema(&mut self, uri: &str) -> RaiseResult<()> {
        if self.by_uri.remove(uri).is_none() {
            raise_error!("ERR_SCHEMA_NOT_FOUND", error = "Schéma introuvable.");
        }
        Ok(())
    }

    #[allow(dead_code)]
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

        self.by_uri.insert(uri.to_string(), schema);
        Ok(())
    }

    #[allow(dead_code)]
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

        self.by_uri.insert(uri.to_string(), schema);
        Ok(())
    }

    #[allow(dead_code)]
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

        self.by_uri.insert(uri.to_string(), schema);
        Ok(())
    }
}

// ============================================================================
// TESTS UNITAIRES (Mis à jour pour le nouveau fonctionnement)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json_db::storage::{file_storage, JsonDbConfig};
    use crate::utils::io::fs::tempdir;
    use crate::utils::testing::mock::inject_mock_config;

    #[async_test]
    async fn test_registry_loading_from_index() -> RaiseResult<()> {
        inject_mock_config().await;

        let dir = match tempdir() {
            Ok(d) => d,
            Err(e) => panic!("Erreur tempdir : {:?}", e),
        };

        let config = JsonDbConfig::new(dir.path().to_path_buf());
        let space = "s1";
        let db = "d1";

        // 🎯 1. Préparation de l'arborescence physique
        let schemas_root = config.db_schemas_root(space, db);
        fs::ensure_dir_async(&schemas_root.join("v1/users")).await?;
        fs::ensure_dir_async(&schemas_root.join("v2/users")).await?;

        // 🎯 2. Création des fichiers schémas réels
        let schema_v1 = json_value!({ "type": "object", "title": "User V1" });
        let schema_v2 = json_value!({ "type": "object", "title": "User V2" });

        fs::write_json_atomic_async(&schemas_root.join("v1/users/user.schema.json"), &schema_v1)
            .await?;
        fs::write_json_atomic_async(&schemas_root.join("v2/users/user.schema.json"), &schema_v2)
            .await?;

        // 🎯 3. On simule un _system.json qui contient les POINTEURS (Nouvelle architecture)
        let mock_system_index = json_value!({
            "schemas": {
                "v1": {
                    "users/user.schema.json": { "file": "v1/users/user.schema.json" }
                },
                "v2": {
                    "users/user.schema.json": { "file": "v2/users/user.schema.json" }
                }
            }
        });

        file_storage::write_system_index(&config, space, db, &mock_system_index).await?;

        // 4. On charge le registre
        let reg = SchemaRegistry::from_db(&config, space, db).await?;

        // 5. Vérifications
        let uri_v1 = format!("db://{}/{}/schemas/v1/users/user.schema.json", space, db);
        let uri_v2 = format!("db://{}/{}/schemas/v2/users/user.schema.json", space, db);

        assert!(
            reg.get_by_uri(&uri_v1).is_some(),
            "Le schéma V1 doit être chargé en mémoire via son fichier physique"
        );
        assert!(
            reg.get_by_uri(&uri_v2).is_some(),
            "Le schéma V2 doit être chargé en mémoire via son fichier physique"
        );

        assert_eq!(reg.get_by_uri(&uri_v2).unwrap()["title"], "User V2");

        Ok(())
    }

    #[async_test]
    async fn test_schema_ddl_operations_in_memory() -> RaiseResult<()> {
        inject_mock_config().await;
        let mut reg = SchemaRegistry::new();
        let uri = "db://test/db/schemas/v2/products/product.schema.json";

        // 1. CREATE
        let initial_schema = json_value!({
            "type": "object",
            "properties": { "name": { "type": "string" } }
        });
        reg.create_schema(uri, initial_schema).await?;
        assert!(reg.get_by_uri(uri).is_some());

        // 2. ADD PROPERTY
        reg.add_property(uri, "price", json_value!({ "type": "number" }))
            .await?;
        assert!(reg.get_by_uri(uri).unwrap()["properties"]["price"].is_object());

        // 3. ALTER PROPERTY
        reg.alter_property(
            uri,
            "price",
            json_value!({ "type": "number", "minimum": 0 }),
        )
        .await?;
        assert_eq!(
            reg.get_by_uri(uri).unwrap()["properties"]["price"]["minimum"],
            0
        );

        // 4. DROP PROPERTY
        reg.drop_property(uri, "name").await?;
        assert!(!reg.get_by_uri(uri).unwrap()["properties"]
            .as_object()
            .unwrap()
            .contains_key("name"));

        // 5. DROP SCHEMA
        reg.drop_schema(uri).await?;
        assert!(reg.get_by_uri(uri).is_none());

        Ok(())
    }
}
