// FICHIER : src-tauri/src/json_db/indexes/hash.rs

use crate::utils::data::HashMap;
use crate::utils::prelude::*;

use super::{driver, paths, IndexDefinition};
use crate::json_db::storage::JsonDbConfig;

#[allow(clippy::too_many_arguments)]
pub async fn update_hash_index(
    cfg: &JsonDbConfig,
    space: &str,
    db: &str,
    collection: &str,
    def: &IndexDefinition,
    doc_id: &str,
    old_doc: Option<&Value>,
    new_doc: Option<&Value>,
) -> RaiseResult<()> {
    let path = paths::index_path(cfg, space, db, collection, &def.name, def.index_type);
    // On spécifie le type concret HashMap pour le driver générique (appel async)
    driver::update::<HashMap<String, Vec<String>>>(&path, def, doc_id, old_doc, new_doc).await
}

/// Recherche des IDs de documents correspondant exactement à une valeur.
pub async fn search_hash_index(
    cfg: &JsonDbConfig,
    space: &str,
    db: &str,
    collection: &str,
    def: &IndexDefinition,
    value: &Value,
) -> RaiseResult<Vec<String>> {
    let path = paths::index_path(cfg, space, db, collection, &def.name, def.index_type);

    // IMPORTANT : La clé stockée dans l'index est la représentation stringifiée du JSON.
    // Ex: Si value est string "admin", key sera "\"admin\"" (avec les guillemets).
    let key = value.to_string();

    // Appel async au driver
    driver::search::<HashMap<String, Vec<String>>>(&path, &key).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json_db::indexes::IndexType;
    // Utilisation de la façade pour les tests
    use crate::utils::{
        io::{self, tempdir}, // fs enrichi + tempdir
        json::json,          // macro json!
    };

    fn setup_env() -> (tempfile::TempDir, JsonDbConfig) {
        let dir = tempdir().unwrap();
        let cfg = JsonDbConfig::new(dir.path().to_path_buf());
        (dir, cfg)
    }

    #[tokio::test] // Migration vers tokio test
    async fn test_hash_lifecycle() {
        let (dir, cfg) = setup_env();
        // Création structure dossiers nécessaire pour le test
        let idx_dir = dir.path().join("s/d/collections/c/_indexes");
        io::ensure_dir(&idx_dir).await.unwrap();

        let def = IndexDefinition {
            name: "email".into(),
            field_path: "/email".into(),
            index_type: IndexType::Hash,
            unique: true,
        };

        // 1. Insertion (Async)
        let doc = json!({ "email": "test@mail.com" });
        update_hash_index(&cfg, "s", "d", "c", &def, "doc1", None, Some(&doc))
            .await
            .unwrap();

        // 2. Recherche (Succès - Async)
        let results = search_hash_index(&cfg, "s", "d", "c", &def, &json!("test@mail.com"))
            .await
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0], "doc1");

        // 3. Recherche (Echec - Async)
        let empty = search_hash_index(&cfg, "s", "d", "c", &def, &json!("other@mail.com"))
            .await
            .unwrap();
        assert!(empty.is_empty());

        // 4. Suppression (Mise à jour vers None - Async)
        update_hash_index(&cfg, "s", "d", "c", &def, "doc1", Some(&doc), None)
            .await
            .unwrap();
        let deleted = search_hash_index(&cfg, "s", "d", "c", &def, &json!("test@mail.com"))
            .await
            .unwrap();
        assert!(deleted.is_empty());
    }
}
