// FICHIER : src-tauri/src/json_db/indexes/hash.rs

use anyhow::Result;
use serde_json::Value;
use std::collections::HashMap;

use super::{driver, paths, IndexDefinition};
use crate::json_db::storage::JsonDbConfig;

#[allow(clippy::too_many_arguments)]
pub fn update_hash_index(
    cfg: &JsonDbConfig,
    space: &str,
    db: &str,
    collection: &str,
    def: &IndexDefinition,
    doc_id: &str,
    old_doc: Option<&Value>,
    new_doc: Option<&Value>,
) -> Result<()> {
    let path = paths::index_path(cfg, space, db, collection, &def.name, def.index_type);
    // On spécifie le type concret HashMap pour le driver générique
    driver::update::<HashMap<String, Vec<String>>>(&path, def, doc_id, old_doc, new_doc)
}

/// Recherche des IDs de documents correspondant exactement à une valeur.
pub fn search_hash_index(
    cfg: &JsonDbConfig,
    space: &str,
    db: &str,
    collection: &str,
    def: &IndexDefinition,
    value: &Value,
) -> Result<Vec<String>> {
    let path = paths::index_path(cfg, space, db, collection, &def.name, def.index_type);

    // IMPORTANT : La clé stockée dans l'index est la représentation stringifiée du JSON.
    // Ex: Si value est string "admin", key sera "\"admin\"" (avec les guillemets).
    // Cela garantit la cohérence avec la méthode update() qui utilise .to_string() sur le Value.
    let key = value.to_string();

    driver::search::<HashMap<String, Vec<String>>>(&path, &key)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json_db::indexes::IndexType;
    use serde_json::json;
    use tempfile::tempdir;

    fn setup_env() -> (tempfile::TempDir, JsonDbConfig) {
        let dir = tempdir().unwrap();
        let cfg = JsonDbConfig::new(dir.path().to_path_buf());
        (dir, cfg)
    }

    #[test]
    fn test_hash_lifecycle() {
        let (dir, cfg) = setup_env();
        // Création structure dossiers nécessaire pour le test
        let idx_dir = dir.path().join("s/d/collections/c/_indexes");
        std::fs::create_dir_all(&idx_dir).unwrap();

        let def = IndexDefinition {
            name: "email".into(),
            field_path: "/email".into(),
            index_type: IndexType::Hash,
            unique: true,
        };

        // 1. Insertion
        let doc = json!({ "email": "test@mail.com" });
        update_hash_index(&cfg, "s", "d", "c", &def, "doc1", None, Some(&doc)).unwrap();

        // 2. Recherche (Succès)
        // Note: On passe la Value brute, la fonction se charge de la sérialiser en clé
        let results =
            search_hash_index(&cfg, "s", "d", "c", &def, &json!("test@mail.com")).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0], "doc1");

        // 3. Recherche (Echec)
        let empty = search_hash_index(&cfg, "s", "d", "c", &def, &json!("other@mail.com")).unwrap();
        assert!(empty.is_empty());

        // 4. Suppression (Mise à jour vers None)
        update_hash_index(&cfg, "s", "d", "c", &def, "doc1", Some(&doc), None).unwrap();
        let deleted =
            search_hash_index(&cfg, "s", "d", "c", &def, &json!("test@mail.com")).unwrap();
        assert!(deleted.is_empty());
    }
}
