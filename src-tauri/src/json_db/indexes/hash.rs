// FICHIER : src-tauri/src/json_db/indexes/hash.rs

use crate::utils::prelude::*;

use super::{driver, paths, IndexDefinition};
// ✅ AJOUT : Import du StorageEngine
use crate::json_db::storage::StorageEngine;

#[allow(clippy::too_many_arguments)]
pub async fn update_hash_index(
    storage: &StorageEngine, // ✅ MODIFIÉ
    space: &str,
    db: &str,
    collection: &str,
    def: &IndexDefinition,
    doc_id: &str,
    old_doc: Option<&JsonValue>,
    new_doc: Option<&JsonValue>,
) -> RaiseResult<()> {
    // ✅ MODIFIÉ : On extrait la config du storage pour le path
    let path = paths::index_path(
        &storage.config,
        space,
        db,
        collection,
        &def.name,
        def.index_type,
    );
    driver::update::<UnorderedMap<String, Vec<String>>>(&path, def, doc_id, old_doc, new_doc).await
}

/// Recherche des IDs de documents correspondant exactement à une valeur.
pub async fn search_hash_index(
    storage: &StorageEngine, // ✅ MODIFIÉ
    space: &str,
    db: &str,
    collection: &str,
    def: &IndexDefinition,
    value: &JsonValue,
) -> RaiseResult<Vec<String>> {
    let path = paths::index_path(
        &storage.config,
        space,
        db,
        collection,
        &def.name,
        def.index_type,
    );

    let key = value.to_string();
    driver::search::<UnorderedMap<String, Vec<String>>>(&path, &key).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json_db::indexes::IndexType;
    use crate::json_db::storage::{JsonDbConfig, StorageEngine};

    fn setup_env() -> (tempfile::TempDir, JsonDbConfig) {
        let dir = tempdir().unwrap();
        let cfg = JsonDbConfig::new(dir.path().to_path_buf());
        (dir, cfg)
    }

    #[async_test]
    async fn test_hash_lifecycle() {
        let (dir, cfg) = setup_env();
        // ✅ AJOUT : Création du StorageEngine pour les tests
        let storage = StorageEngine::new(cfg);

        let idx_dir = dir.path().join("s/d/collections/c/_indexes");
        fs::ensure_dir_async(&idx_dir).await.unwrap();

        let def = IndexDefinition {
            name: "email".into(),
            field_path: "/email".into(),
            index_type: IndexType::Hash,
            unique: true,
        };

        // 1. Insertion
        let doc = json_value!({ "email": "test@mail.com" });
        update_hash_index(&storage, "s", "d", "c", &def, "doc1", None, Some(&doc))
            .await
            .unwrap();

        // 2. Recherche (Succès)
        let results =
            search_hash_index(&storage, "s", "d", "c", &def, &json_value!("test@mail.com"))
                .await
                .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0], "doc1");

        // 3. Recherche (Echec)
        let empty = search_hash_index(
            &storage,
            "s",
            "d",
            "c",
            &def,
            &json_value!("other@mail.com"),
        )
        .await
        .unwrap();
        assert!(empty.is_empty());

        // 4. Suppression
        update_hash_index(&storage, "s", "d", "c", &def, "doc1", Some(&doc), None)
            .await
            .unwrap();
        let deleted =
            search_hash_index(&storage, "s", "d", "c", &def, &json_value!("test@mail.com"))
                .await
                .unwrap();
        assert!(deleted.is_empty());
    }
}
