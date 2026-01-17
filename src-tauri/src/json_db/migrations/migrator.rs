// FICHIER : src-tauri/src/json_db/migrations/migrator.rs

use super::version::MigrationVersion;
use super::{Migration, MigrationStep};
use crate::json_db::collections::manager::CollectionsManager;
use crate::json_db::storage::StorageEngine;

use anyhow::Result;
use chrono::Utc;
use serde_json::{json, Value};
use std::collections::HashSet;

pub struct Migrator<'a> {
    manager: CollectionsManager<'a>,
}

impl<'a> Migrator<'a> {
    pub fn new(storage: &'a StorageEngine, space: &str, db: &str) -> Self {
        Self {
            manager: CollectionsManager::new(storage, space, db),
        }
    }

    /// Initialise la table de suivi des migrations (_migrations) - ASYNC
    pub async fn init(&self) -> Result<()> {
        let exists = self
            .manager
            .list_collections()
            .await? // Migration async
            .contains(&"_migrations".to_string());
        if !exists {
            #[cfg(debug_assertions)]
            println!("⚙️ Création de la table de suivi des migrations...");
            self.manager.create_collection("_migrations", None).await?;
        }
        Ok(())
    }

    /// Exécute les migrations en attente - ASYNC
    pub async fn run_migrations(&self, declared_migrations: Vec<Migration>) -> Result<()> {
        self.init().await?;

        // 1. Récupérer les migrations déjà appliquées
        let applied_docs = self.manager.list_all("_migrations").await?;
        let applied_ids: HashSet<String> = applied_docs
            .iter()
            .filter_map(|doc| {
                doc.get("id")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
            })
            .collect();

        // 2. Trier les migrations déclarées par version
        let mut sorted_migrations = declared_migrations;
        sorted_migrations.sort_by(|a, b| {
            let ver_a = MigrationVersion::parse(&a.version)
                .unwrap_or(MigrationVersion::parse("0.0.0").unwrap());
            let ver_b = MigrationVersion::parse(&b.version)
                .unwrap_or(MigrationVersion::parse("0.0.0").unwrap());
            ver_a.cmp(&ver_b)
        });

        // 3. Appliquer celles qui manquent
        for migration in sorted_migrations {
            if !applied_ids.contains(&migration.id) {
                println!(
                    "🚀 Application de la migration {} ({})",
                    migration.version, migration.description
                );
                self.apply_migration(&migration).await?;
            }
        }

        Ok(())
    }

    async fn apply_migration(&self, migration: &Migration) -> Result<()> {
        // Exécution atomique des étapes (Up)
        for step in &migration.up {
            self.execute_step(step).await?;
        }

        // Enregistrement du succès
        let record = json!({
            "id": migration.id,
            "version": migration.version,
            "description": migration.description,
            "appliedAt": Utc::now().to_rfc3339()
        });

        self.manager.insert_raw("_migrations", &record).await?;

        Ok(())
    }

    async fn execute_step(&self, step: &MigrationStep) -> Result<()> {
        match step {
            MigrationStep::CreateCollection { name, schema } => {
                let schema_str = schema.as_str().map(|s| s.to_string());
                self.manager.create_collection(name, schema_str).await?;
                println!("   -> Collection créée : {}", name);
            }
            MigrationStep::DropCollection { name } => {
                self.manager.drop_collection(name).await?;
                println!("   -> Collection supprimée : {}", name);
            }
            MigrationStep::CreateIndex { collection, fields } => {
                if let Some(field) = fields.first() {
                    self.manager
                        .create_index(collection, field, "btree")
                        .await?;
                    println!("   -> Index créé sur {}::{}", collection, field);
                }
            }
            MigrationStep::DropIndex { collection, name } => {
                self.manager.drop_index(collection, name).await?;
                println!("   -> Index supprimé sur {}::{}", collection, name);
            }
            MigrationStep::AddField {
                collection,
                field,
                default,
            } => {
                self.transform_all_documents(collection, |doc| {
                    if let Some(obj) = doc.as_object_mut() {
                        if !obj.contains_key(field) {
                            obj.insert(field.clone(), default.clone().unwrap_or(Value::Null));
                            return true;
                        }
                    }
                    false
                })
                .await?;
                println!("   -> Champ ajouté : {}::{}", collection, field);
            }
            MigrationStep::RemoveField { collection, field } => {
                self.transform_all_documents(collection, |doc| {
                    if let Some(obj) = doc.as_object_mut() {
                        if obj.remove(field).is_some() {
                            return true;
                        }
                    }
                    false
                })
                .await?;
                println!("   -> Champ supprimé : {}::{}", collection, field);
            }
            MigrationStep::RenameField {
                collection,
                old_name,
                new_name,
            } => {
                self.transform_all_documents(collection, |doc| {
                    if let Some(obj) = doc.as_object_mut() {
                        if let Some(val) = obj.remove(old_name) {
                            obj.insert(new_name.clone(), val);
                            return true;
                        }
                    }
                    false
                })
                .await?;
                println!(
                    "   -> Champ renommé : {}::{} -> {}",
                    collection, old_name, new_name
                );
            }
        }
        Ok(())
    }

    async fn transform_all_documents<F>(&self, collection: &str, mut transformer: F) -> Result<()>
    where
        F: FnMut(&mut Value) -> bool,
    {
        let docs = self.manager.list_all(collection).await?;

        for mut doc in docs {
            let id = doc.get("id").and_then(|v| v.as_str()).unwrap().to_string();

            if transformer(&mut doc) {
                self.manager.update_document(collection, &id, doc).await?;
            }
        }
        Ok(())
    }
}

// ============================================================================
// TESTS D'INTÉGRATION
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json_db::migrations::{Migration, MigrationStep};
    use crate::json_db::storage::{JsonDbConfig, StorageEngine};
    use serde_json::json;
    use tempfile::tempdir;

    fn create_test_env() -> (StorageEngine, tempfile::TempDir) {
        let temp_dir = tempdir().expect("Impossible de créer dossier temp DB");
        let config = JsonDbConfig::new(temp_dir.path().to_path_buf());
        let storage = StorageEngine::new(config);
        (storage, temp_dir)
    }

    #[tokio::test]
    async fn test_migration_lifecycle() {
        let (storage, _dir) = create_test_env();
        let space = "test_space";
        let db = "test_db";
        let migrator = Migrator::new(&storage, space, db);

        let m1 = Migration {
            id: "m1".to_string(),
            version: "1.0.0".to_string(),
            description: "Init Users".to_string(),
            up: vec![MigrationStep::CreateCollection {
                name: "users".to_string(),
                schema: json!(null),
            }],
            down: vec![],
            applied_at: None,
        };

        migrator
            .run_migrations(vec![m1.clone()])
            .await
            .expect("Migration 1 failed");

        let cols = migrator.manager.list_collections().await.unwrap();
        assert!(cols.contains(&"users".to_string()));

        let mig_docs = migrator.manager.list_all("_migrations").await;
        assert!(mig_docs.is_ok());

        let user_doc = json!({ "id": "user_1", "name": "Alice" });
        migrator
            .manager
            .insert_raw("users", &user_doc)
            .await
            .expect("Insert failed");

        let m2 = Migration {
            id: "m2".to_string(),
            version: "1.1.0".to_string(),
            description: "Add Active Field".to_string(),
            up: vec![MigrationStep::AddField {
                collection: "users".to_string(),
                field: "active".to_string(),
                default: Some(json!(true)),
            }],
            down: vec![],
            applied_at: None,
        };

        migrator
            .run_migrations(vec![m1, m2])
            .await
            .expect("Migration 2 failed");

        let updated_doc = migrator
            .manager
            .get("users", "user_1")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(updated_doc["active"], true);
        assert_eq!(updated_doc["name"], "Alice");

        let history = migrator.manager.list_all("_migrations").await.unwrap();
        assert_eq!(history.len(), 2);
    }

    #[tokio::test]
    async fn test_rename_field() {
        let (storage, _dir) = create_test_env();
        let migrator = Migrator::new(&storage, "space", "db");
        // Init DB requis pour les fonctions internes
        migrator.manager.init_db().await.unwrap();

        migrator
            .manager
            .create_collection("products", None)
            .await
            .unwrap();
        migrator
            .manager
            .insert_raw("products", &json!({"id": "p1", "cost": 100}))
            .await
            .unwrap();

        let m_rename = Migration {
            id: "rename_01".to_string(),
            version: "1.0.0".to_string(),
            description: "Rename cost to price".to_string(),
            up: vec![MigrationStep::RenameField {
                collection: "products".to_string(),
                old_name: "cost".to_string(),
                new_name: "price".to_string(),
            }],
            down: vec![],
            applied_at: None,
        };

        migrator.run_migrations(vec![m_rename]).await.unwrap();

        let doc = migrator
            .manager
            .get("products", "p1")
            .await
            .unwrap()
            .unwrap();
        assert!(doc.get("cost").is_none());
        assert_eq!(doc["price"], 100);
    }
}
