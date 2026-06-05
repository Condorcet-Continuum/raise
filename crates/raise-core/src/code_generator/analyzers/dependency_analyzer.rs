// FICHIER : crates/raise-core/src/code_generator/analyzers/dependency_analyzer.rs

use crate::json_db::collections::manager::CollectionsManager;
use crate::json_db::query::{Condition, FilterOperator, Query, QueryEngine, QueryFilter};
use crate::json_db::transactions::{manager::TransactionManager, TransactionRequest};
use crate::utils::prelude::*; // 🎯 Façade Unique RAISE

pub struct DependencyAnalyzer<'a> {
    manager: &'a CollectionsManager<'a>,
}

impl<'a> DependencyAnalyzer<'a> {
    pub fn new(manager: &'a CollectionsManager<'a>) -> Self {
        Self { manager }
    }

    /// 🧠 Parcourt un module spécifique pour transformer la syntaxe (raw_imports) en sémantique (dependencies)
    pub async fn link_module(&self, collection: &str, module_id: &str) -> RaiseResult<usize> {
        let query_engine = QueryEngine::new(self.manager);

        // 🎯 1. Isolation stricte : On ne récupère que les éléments du module cible
        let mut query = Query::new(collection);
        query.filter = Some(QueryFilter {
            operator: FilterOperator::And,
            conditions: vec![Condition::eq("module_id", json_value!(module_id))],
        });

        let result = query_engine.execute_query(query).await?;

        let mut updates = Vec::new();
        let mut resolved_count = 0;

        for mut doc in result.documents {
            let id = match doc.get("_id").and_then(|v| v.as_str()) {
                Some(i) => i.to_string(),
                None => continue,
            };

            let mut new_dependencies = Vec::new();
            let mut imports_to_keep = Vec::new();
            let mut modified = false;

            // 2. Extraire les imports bruts des métadonnées
            if let Some(meta) = doc.get_mut("metadata").and_then(|m| m.as_object_mut()) {
                if let Some(raw_imports) = meta.get("raw_imports").and_then(|v| v.as_array()) {
                    for import_val in raw_imports {
                        if let Some(import_str) = import_val.as_str() {
                            // Ex: "crate::json_db::storage::StorageEngine" -> "StorageEngine"
                            let target_name = import_str.split("::").last().unwrap_or(import_str);

                            // 3. Chercher la cible en base de données (recherche globale sur toute la base)
                            if let Some(target_id) = self
                                .find_element_id(&query_engine, collection, target_name)
                                .await?
                            {
                                new_dependencies.push(json_value!(target_id));
                                resolved_count += 1;
                                modified = true;
                            } else {
                                // 🎯 Résilience : On conserve l'import s'il pointe vers un module pas encore ingéré
                                imports_to_keep.push(import_val.clone());
                            }
                        }
                    }

                    // Mise à jour de la liste des imports restants
                    if modified {
                        meta.insert("raw_imports".to_string(), json_value!(imports_to_keep));
                    }
                }
            }

            // 4. Préparer la transaction d'Update si des liens ont été trouvés
            if modified {
                // 🎯 FIX Erreur 1 (E0599) : On convertit le JsonValue en Object_mut avant d'insérer
                if let Some(deps) = doc.get_mut("dependencies").and_then(|d| d.as_array_mut()) {
                    for new_dep in new_dependencies {
                        if !deps.contains(&new_dep) {
                            deps.push(new_dep);
                        }
                    }
                } else if let Some(obj) = doc.as_object_mut() {
                    obj.insert("dependencies".to_string(), json_value!(new_dependencies));
                }

                // 🎯 FIX Erreur 3 (E0063) : Le type Update exige de connaître le handle
                let handle = doc
                    .get("handle")
                    .and_then(|h| h.as_str())
                    .map(|s| s.to_string());

                updates.push(TransactionRequest::Update {
                    collection: collection.to_string(),
                    // 🎯 FIX Erreur 2 (E0308) : L'identifiant doit être enveloppé dans un Option
                    id: Some(id),
                    handle,
                    document: doc,
                });
            }
        }

        // 5. Exécution transactionnelle atomique par lot
        if !updates.is_empty() {
            let tx_mgr = TransactionManager::new(
                self.manager.storage,
                &self.manager.space,
                &self.manager.db,
            );
            tx_mgr.execute_smart(updates).await?;
        }

        Ok(resolved_count)
    }

    /// 🔍 Moteur de recherche heuristique pour trouver l'_id d'un élément par son nom
    async fn find_element_id(
        &self,
        query_engine: &QueryEngine<'_>,
        collection: &str,
        target_name: &str,
    ) -> RaiseResult<Option<String>> {
        let mut query = Query::new(collection);
        query.filter = Some(QueryFilter {
            operator: FilterOperator::And,
            // 🎯 FIX Erreur 4 (E0308) : On emballe le &str dans un json_value!
            conditions: vec![Condition::contains("handle", json_value!(target_name))],
        });

        let result = query_engine.execute_query(query).await?;
        Ok(result.documents.first().and_then(|doc| {
            doc.get("_id")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        }))
    }
}

// =========================================================================
// TESTS UNITAIRES (Validation du Tissage Sémantique)
// =========================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::testing::DbSandbox;

    #[async_test]
    async fn test_dependency_analyzer_link_module() -> RaiseResult<()> {
        let sandbox = DbSandbox::new().await?;
        let manager = CollectionsManager::new(&sandbox.storage, "test_domain", "test_db");

        let _ = DbSandbox::mock_db(&manager).await;

        // 1. Préparation de la Sandbox
        let schema_uri = "db://test_domain/test_db/schemas/v1/db/generic.schema.json";
        manager
            .create_collection("code_elements", schema_uri)
            .await?;

        // 2. Insertion de la CIBLE (Le module qui est importé)
        manager
            .upsert_document(
                "code_elements",
                json_value!({
                    "_id": "uuid-target-123",
                    "module_id": "mod_target",
                    "handle": "struct:StorageEngine",
                    "element_type": "struct"
                }),
            )
            .await?;

        // 3. Insertion de la SOURCE (L'élément qui importe, appartenant à mod_source)
        // On y place un import valide (StorageEngine) et un import fantôme (Ghost)
        manager
            .upsert_document(
                "code_elements",
                json_value!({
                    "_id": "uuid-source-456",
                    "module_id": "mod_source",
                    "handle": "fn:init_db",
                    "element_type": "function",
                    "dependencies": [],
                    "metadata": {
                        "raw_imports": [
                            "crate::json_db::storage::StorageEngine",
                            "crate::unknown::Ghost"
                        ]
                    }
                }),
            )
            .await?;

        // 4. Exécution ciblée de l'analyseur UNIQUEMENT sur 'mod_source'
        let analyzer = DependencyAnalyzer::new(&manager);
        let resolved = analyzer.link_module("code_elements", "mod_source").await?;

        // 5. Assertions
        assert_eq!(
            resolved, 1,
            "Il devrait y avoir exactement 1 dépendance résolue"
        );

        // Vérification de la mutation en base
        let updated_doc = manager
            .get_document("code_elements", "uuid-source-456")
            .await?
            .unwrap();

        // A) Le StorageEngine a dû être basculé en véritable UUID dans 'dependencies'
        let deps = updated_doc["dependencies"].as_array().unwrap();
        assert!(deps.contains(&json_value!("uuid-target-123")));

        // B) Le composant Ghost a dû rester dans 'raw_imports' en attendant que son fichier soit ingéré
        let raw_imports = updated_doc["metadata"]["raw_imports"].as_array().unwrap();
        assert_eq!(raw_imports.len(), 1);
        assert_eq!(raw_imports[0].as_str().unwrap(), "crate::unknown::Ghost");

        Ok(())
    }
}
