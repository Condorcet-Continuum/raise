// FICHIER : src-tauri/src/json_db/migrations/migrator.rs

use super::version::MigrationVersion;
use super::{Migration, MigrationStep};
use crate::json_db::collections::manager::CollectionsManager;
use crate::json_db::storage::StorageEngine;

use crate::utils::prelude::*;

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
    pub async fn init(&self) -> RaiseResult<()> {
        let exists = self
            .manager
            .list_collections()
            .await? // Migration async
            .contains(&"_migrations".to_string());

        if !exists {
            #[cfg(debug_assertions)]
            println!("⚙️ Création de la table de suivi des migrations...");

            self.manager
                .create_collection(
                    "_migrations",
                    "db://_system/_system/schemas/v1/db/generic.schema.json",
                )
                .await?;
        }
        Ok(())
    }

    /// Exécute les migrations en attente - ASYNC
    pub async fn run_migrations(&self, mut declared_migrations: Vec<Migration>) -> RaiseResult<()> {
        // 1. Initialisation de la table de suivi
        self.init().await?;

        // 2. Validation préalable de TOUTES les versions déclarées
        // On évite ainsi de découvrir une erreur de frappe à la moitié du processus.
        for m in &declared_migrations {
            MigrationVersion::parse(&m.version)?;
        }

        // 3. Tri chronologique sécurisé (SemVer)
        declared_migrations.sort_by(|a, b| {
            let v_a = MigrationVersion::parse(&a.version).unwrap(); // Garanti par l'étape 2
            let v_b = MigrationVersion::parse(&b.version).unwrap();
            v_a.cmp(&v_b)
        });

        // 4. Identification des migrations déjà appliquées
        let applied_docs = self.manager.list_all("_migrations").await?;
        let applied_ids: UniqueSet<String> = applied_docs
            .iter()
            .filter_map(|doc| {
                doc.get("_id")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
            })
            .collect();

        // 5. Application séquentielle
        for migration in declared_migrations {
            if !applied_ids.contains(&migration.id) {
                #[cfg(debug_assertions)]
                println!(
                    "🚀 Migration : {} - {}",
                    migration.version, migration.description
                );

                self.apply_migration(&migration).await?;
            }
        }

        Ok(())
    }

    async fn apply_migration(&self, migration: &Migration) -> RaiseResult<()> {
        // Exécution atomique des étapes (Up)
        for step in &migration.up {
            self.execute_step(step).await?;
        }

        // Enregistrement du succès
        let record = json_value!({
            "_id": migration.id.clone(),
            "$schema": "db://_system/_system/schemas/v2/system/db/migration.schema.json",
            "handle": format!("migration_{}", migration.version).replace('.', "_"),
            "name": { "fr": migration.description.clone(), "en": migration.description.clone() },
            "version": migration.version.clone(),
            "description": migration.description.clone(),
            "applied_at": UtcClock::now().to_rfc3339()
        });

        self.manager.insert_raw("_migrations", &record).await?;

        Ok(())
    }

    async fn execute_step(&self, step: &MigrationStep) -> RaiseResult<()> {
        match step {
            // 1. Création d'une nouvelle collection
            MigrationStep::CreateCollection { name, schema } => {
                let schema_str = match schema.as_str() {
                    Some(s) => s,
                    None => raise_error!(
                        "ERR_MIGRATION_SCHEMA_MISSING",
                        error = format!(
                            "Le schéma est obligatoire pour créer la collection '{}'.",
                            name
                        ),
                        context = json_value!({
                            "collection": name,
                            "hint": "Le champ 'schema' doit être une URI (chaîne de caractères)."
                        })
                    ),
                };
                self.manager.create_collection(name, schema_str).await?;
                #[cfg(debug_assertions)]
                println!("   -> Collection créée : {}", name);
            }

            // 2. Suppression d'une collection
            MigrationStep::DropCollection { name } => {
                self.manager.drop_collection(name).await?;
                #[cfg(debug_assertions)]
                println!("   -> Collection supprimée : {}", name);
            }

            // 3. Ajout d'un champ à tous les documents
            MigrationStep::AddField {
                collection,
                field,
                default,
            } => {
                self.transform_all_documents(collection, |doc| {
                    if let Some(obj) = doc.as_object_mut() {
                        if !obj.contains_key(field) {
                            let default_val = default.clone().unwrap_or(JsonValue::Null);
                            obj.insert(field.clone(), default_val);
                            return true;
                        }
                    }
                    false
                })
                .await?;
                #[cfg(debug_assertions)]
                println!("   -> Champ ajouté : {}::{}", collection, field);
            }

            // 4. Suppression d'un champ dans tous les documents
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
                #[cfg(debug_assertions)]
                println!("   -> Champ supprimé : {}::{}", collection, field);
            }

            // 5. Renommage d'un champ
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
                #[cfg(debug_assertions)]
                println!(
                    "   -> Champ renommé : {}::{} -> {}",
                    collection, old_name, new_name
                );
            }

            // 6. Création d'un index
            MigrationStep::CreateIndex { collection, fields } => {
                if let Some(field) = fields.first() {
                    self.manager
                        .create_index(collection, field, "btree")
                        .await?;
                    #[cfg(debug_assertions)]
                    println!("   -> Index btree créé sur {}::{}", collection, field);
                } else {
                    raise_error!(
                        "ERR_MIGRATION_INDEX_EMPTY",
                        error = format!(
                            "Impossible de créer un index sans champs sur '{}'.",
                            collection
                        )
                    );
                }
            }

            // 7. Suppression d'un index
            MigrationStep::DropIndex { collection, name } => {
                self.manager.drop_index(collection, name).await?;
                #[cfg(debug_assertions)]
                println!("   -> Index supprimé : {}::{}", collection, name);
            }

            // 8. Logique personnalisée (Custom)
            MigrationStep::Custom { handler, params } => {
                #[cfg(debug_assertions)]
                println!("   -> Exécution du handler Custom : '{}'", handler);

                // Pour l'instant, on lève une erreur explicite si le handler n'est pas reconnu.
                // Cela évite une migration silencieusement ignorée (Dette Technique).
                match handler.as_str() {
                    "noop" => Ok::<(), AppError>(()), // Handler de test
                    _ => raise_error!(
                        "ERR_MIGRATION_CUSTOM_HANDLER_NOT_FOUND",
                        error = format!("Le handler de migration '{}' est inconnu.", handler),
                        context = json_value!({ "params": params })
                    ),
                }?;
            }
        }
        Ok(())
    }

    async fn transform_all_documents<F>(
        &self,
        collection: &str,
        mut transformer: F,
    ) -> RaiseResult<()>
    where
        F: FnMut(&mut JsonValue) -> bool,
    {
        let docs = self.manager.list_all(collection).await?;

        for mut doc in docs {
            if transformer(&mut doc) {
                // Utilisation de insert_with_schema pour ÉCRASER le document physiquement
                self.manager.insert_with_schema(collection, doc).await?;
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
    use crate::utils::testing::DbSandbox;

    #[async_test]
    async fn test_migration_lifecycle() -> RaiseResult<()> {
        let sandbox = DbSandbox::new().await?;

        let migrator = Migrator::new(
            &sandbox.storage,
            &sandbox.config.mount_points.system.domain,
            &sandbox.config.mount_points.system.db,
        );

        DbSandbox::mock_db(&migrator.manager).await?;

        let m1 = Migration {
            id: "m1".to_string(),
            version: "1.0.0".to_string(),
            description: "Init Users".to_string(),
            up: vec![MigrationStep::CreateCollection {
                name: "users".to_string(),
                schema: json_value!("db://_system/_system/schemas/v1/db/generic.schema.json"),
            }],
            down: vec![],
            applied_at: None,
        };

        migrator.run_migrations(vec![m1.clone()]).await?;

        let cols = migrator.manager.list_collections().await?;
        assert!(cols.contains(&"users".to_string()));

        let mig_docs = migrator.manager.list_all("_migrations").await;
        assert!(mig_docs.is_ok());

        let user_doc = json_value!({ "_id": "user_1", "name": "Alice" });
        migrator.manager.insert_raw("users", &user_doc).await?;

        let m2 = Migration {
            id: "m2".to_string(),
            version: "1.1.0".to_string(),
            description: "Add Active Field".to_string(),
            up: vec![MigrationStep::AddField {
                collection: "users".to_string(),
                field: "active".to_string(),
                default: Some(json_value!(true)),
            }],
            down: vec![],
            applied_at: None,
        };

        migrator.run_migrations(vec![m1, m2]).await?;

        let updated_doc_opt = migrator.manager.get("users", "user_1").await?;
        let updated_doc = match updated_doc_opt {
            Some(d) => d,
            None => panic!("Document utilisateur introuvable après migration"),
        };

        assert_eq!(updated_doc["active"], true);
        assert_eq!(updated_doc["name"], "Alice");

        let history = migrator.manager.list_all("_migrations").await?;

        // 🎯 FIX : On attend 3 migrations (1 Bootstrap + m1 + m2)
        assert_eq!(
            history.len(),
            3,
            "L'historique doit contenir le Bootstrap initial et les 2 migrations du test"
        );

        // Bonus de vérification : on s'assure que m1 et m2 sont bien là
        let ids: Vec<String> = history
            .into_iter()
            .filter_map(|doc| {
                doc.get("_id")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
            })
            .collect();
        assert!(ids.contains(&"m1".to_string()));
        assert!(ids.contains(&"m2".to_string()));

        Ok(())
    }

    #[async_test]
    async fn test_rename_field() -> RaiseResult<()> {
        let sandbox = DbSandbox::new().await?;

        let migrator = Migrator::new(
            &sandbox.storage,
            &sandbox.config.mount_points.system.domain,
            &sandbox.config.mount_points.system.db,
        );

        DbSandbox::mock_db(&migrator.manager).await?;

        migrator
            .manager
            .create_collection(
                "products",
                "db://_system/_system/schemas/v1/db/generic.schema.json",
            )
            .await?;

        migrator
            .manager
            .insert_raw("products", &json_value!({"_id": "p1", "cost": 100}))
            .await?;

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

        migrator.run_migrations(vec![m_rename]).await?;

        let doc_opt = migrator.manager.get("products", "p1").await?;
        let doc = match doc_opt {
            Some(d) => d,
            None => panic!("Produit introuvable après renommage"),
        };

        assert!(doc.get("cost").is_none());
        assert_eq!(doc["price"], 100);

        Ok(())
    }
}
