// FICHIER : src-tauri/src/json_db/schema/bootstrapper.rs

use async_recursion::async_recursion;
use std::path::Path;

use crate::json_db::collections::manager::CollectionsManager;
use crate::json_db::storage::file_storage;
use crate::utils::prelude::*;

/// Le Bootstrapper est responsable de l'installation initiale des schémas DDL.
/// Il lit les fichiers physiques legacy et les intègre définitivement
/// dans l'index de la base de données (Single Source of Truth).
pub struct SchemaBootstrapper<'a> {
    manager: &'a CollectionsManager<'a>,
}

impl<'a> SchemaBootstrapper<'a> {
    pub fn new(manager: &'a CollectionsManager<'a>) -> Self {
        Self { manager }
    }

    /// Exécute l'aspiration des schémas depuis le dossier physique vers l'index.
    pub async fn run(&self, legacy_space: &str, legacy_db: &str) -> RaiseResult<usize> {
        let config = &self.manager.storage.config;
        let legacy_dir = config.db_schemas_root(legacy_space, legacy_db);

        // S'il n'y a pas de dossier physique, on n'a rien à amorcer.
        if !fs::exists_async(&legacy_dir).await {
            return Ok(0);
        }

        let mut sys_doc = self.manager.load_index().await?;
        let mut count = 0;

        // On sécurise l'existence du bloc "schemas" dans le JSON
        if sys_doc.get("schemas").is_none() {
            sys_doc["schemas"] = json_value!({});
        }

        // Parcours récursif et injection
        count += self
            .scan_recursive(&legacy_dir, &legacy_dir, &mut sys_doc)
            .await?;

        if count > 0 {
            // On sauvegarde l'index enrichi directement via file_storage
            // pour contourner les validations métier qui pourraient échouer
            // tant que les schémas ne sont pas encore en mémoire.
            file_storage::write_system_index(
                config,
                &self.manager.space,
                &self.manager.db,
                &sys_doc,
            )
            .await?;

            user_info!(
                "BOOTSTRAP_SCHEMAS_SUCCESS",
                json_value!({
                    "schemas_injected": count,
                    "target_space": self.manager.space,
                    "target_db": self.manager.db
                })
            );
        }

        Ok(count)
    }

    #[async_recursion]
    async fn scan_recursive(
        &self,
        root_dir: &Path,
        current_dir: &Path,
        sys_doc: &mut JsonValue,
    ) -> RaiseResult<usize> {
        let mut count = 0;
        let mut entries = fs::read_dir_async(current_dir).await?;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            let file_type = entry.file_type().await?;

            if file_type.is_dir() {
                count += self.scan_recursive(root_dir, &path, sys_doc).await?;
            } else if file_type.is_file()
                && path.extension().and_then(|s| s.to_str()) == Some("json")
            {
                let content = fs::read_to_string_async(&path).await?;

                if let Ok(schema_json) = json::deserialize_from_str::<JsonValue>(&content) {
                    // Calcul de la clé relative (ex: "v2/identity/session.schema.json")
                    if let Ok(rel_path) = path.strip_prefix(root_dir) {
                        let rel_str = rel_path.to_string_lossy().replace('\\', "/");
                        let parts: Vec<&str> = rel_str.splitn(2, '/').collect();

                        if parts.len() == 2 {
                            let version = parts[0]; // ex: "v2"
                            let schema_key = parts[1]; // ex: "identity/session.schema.json"

                            if sys_doc["schemas"].get(version).is_none() {
                                sys_doc["schemas"][version] = json_value!({});
                            }

                            // Injection atomique dans l'objet
                            if let Some(v_obj) = sys_doc["schemas"][version].as_object_mut() {
                                v_obj.insert(schema_key.to_string(), schema_json);
                                count += 1;
                            }
                        }
                    }
                }
            }
        }
        Ok(count)
    }
}

// ============================================================================
// TESTS UNITAIRES (Robustesse garantie)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::testing::DbSandbox;

    #[async_test]
    async fn test_bootstrapper_injects_legacy_schemas() -> RaiseResult<()> {
        // 1. Initialisation de l'environnement de test
        let sandbox = DbSandbox::new().await;
        let manager = CollectionsManager::new(&sandbox.storage, "test_space", "test_db");

        // On force la création de l'index de base
        DbSandbox::mock_db(&manager).await?;

        // 2. Création d'une arborescence legacy simulée
        let legacy_dir = sandbox
            .storage
            .config
            .db_schemas_root("legacy_space", "legacy_db");
        let v2_dir = legacy_dir.join("v2").join("identity");
        fs::ensure_dir_async(&v2_dir).await?;

        // Création d'un faux fichier schéma JSON
        let fake_schema = json_value!({ "type": "object", "title": "Test Schema" });
        fs::write_json_atomic_async(&v2_dir.join("user.schema.json"), &fake_schema).await?;

        // 3. Exécution du Bootstrapper
        let bootstrapper = SchemaBootstrapper::new(&manager);
        let injected_count = bootstrapper.run("legacy_space", "legacy_db").await?;

        // 4. Vérifications
        assert_eq!(
            injected_count, 1,
            "Un schéma aurait dû être détecté et injecté"
        );

        // On recharge l'index pour vérifier qu'il a bien été muté physiquement
        let sys_doc = manager.load_index().await?;

        let schemas_block = sys_doc
            .get("schemas")
            .expect("Le bloc schemas doit exister");
        let v2_block = schemas_block.get("v2").expect("Le bloc v2 doit exister");
        let user_schema = v2_block
            .get("identity/user.schema.json")
            .expect("Le schéma user doit être injecté");

        assert_eq!(
            user_schema["title"], "Test Schema",
            "Le contenu du schéma doit correspondre"
        );

        Ok(())
    }

    #[async_test]
    async fn test_bootstrapper_handles_missing_legacy_dir() -> RaiseResult<()> {
        let sandbox = DbSandbox::new().await;
        let manager = CollectionsManager::new(&sandbox.storage, "test_space", "test_db");

        let bootstrapper = SchemaBootstrapper::new(&manager);

        // On tente d'importer depuis un domaine qui n'a pas de dossier physique
        let injected_count = bootstrapper.run("ghost_space", "ghost_db").await?;

        assert_eq!(
            injected_count, 0,
            "Doit retourner 0 si le dossier n'existe pas, sans crasher"
        );
        Ok(())
    }
}
