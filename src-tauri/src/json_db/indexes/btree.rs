// FICHIER : src-tauri/src/json_db/indexes/btree.rs

// FAÇADE UNIQUE
use crate::utils::{
    error::AnyResult, // Gestion erreur unifiée
    json::Value,      // JSON unifié
    BTreeMap,
};

use super::{driver, paths, IndexDefinition};
use crate::json_db::storage::JsonDbConfig;

#[allow(clippy::too_many_arguments)]
pub async fn update_btree_index(
    cfg: &JsonDbConfig,
    space: &str,
    db: &str,
    collection: &str,
    def: &IndexDefinition,
    doc_id: &str,
    old_doc: Option<&Value>,
    new_doc: Option<&Value>,
) -> AnyResult<()> {
    let path = paths::index_path(cfg, space, db, collection, &def.name, def.index_type);
    // Appel au driver qui doit être async
    driver::update::<BTreeMap<String, Vec<String>>>(&path, def, doc_id, old_doc, new_doc).await
}

/// Recherche exacte via l'index BTree.
/// Note: La structure BTree permettra à l'avenir des recherches par plage (Range Search)
/// mais pour l'instant nous exposons une recherche exacte standard.
pub async fn search_btree_index(
    cfg: &JsonDbConfig,
    space: &str,
    db: &str,
    collection: &str,
    def: &IndexDefinition,
    value: &Value,
) -> AnyResult<Vec<String>> {
    let path = paths::index_path(cfg, space, db, collection, &def.name, def.index_type);
    let key = value.to_string();
    // Appel au driver qui doit être async
    driver::search::<BTreeMap<String, Vec<String>>>(&path, &key).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json_db::indexes::IndexType;
    use crate::utils::{
        fs::{self, tempdir}, // tempdir et fs:: sont exportés ici
        json::json,          // La macro json! et le module json sont ici
    };

    fn setup_env() -> (tempfile::TempDir, JsonDbConfig) {
        let dir = tempdir().unwrap();
        let cfg = JsonDbConfig::new(dir.path().to_path_buf());
        (dir, cfg)
    }

    #[tokio::test] // Migration vers tokio test
    async fn test_btree_lifecycle() {
        let (dir, cfg) = setup_env();
        let idx_dir = dir.path().join("s/d/collections/c/_indexes");
        fs::create_dir_all(&idx_dir).await.unwrap();

        let def = IndexDefinition {
            name: "age".into(),
            field_path: "/age".into(),
            index_type: IndexType::BTree,
            unique: false,
        };

        // 1. Insertion (Async)
        let doc1 = json!({ "age": 30 });
        update_btree_index(&cfg, "s", "d", "c", &def, "u1", None, Some(&doc1))
            .await
            .unwrap();

        let doc2 = json!({ "age": 25 });
        update_btree_index(&cfg, "s", "d", "c", &def, "u2", None, Some(&doc2))
            .await
            .unwrap();

        // 2. Recherche Exacte (Async)
        let results = search_btree_index(&cfg, "s", "d", "c", &def, &json!(30))
            .await
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0], "u1");

        let results_25 = search_btree_index(&cfg, "s", "d", "c", &def, &json!(25))
            .await
            .unwrap();
        assert_eq!(results_25.len(), 1);
        assert_eq!(results_25[0], "u2");

        // 3. Recherche vide (Async)
        let results_empty = search_btree_index(&cfg, "s", "d", "c", &def, &json!(99))
            .await
            .unwrap();
        assert!(results_empty.is_empty());
    }
}
