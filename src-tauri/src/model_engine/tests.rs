// FICHIER : src-tauri/src/model_engine/tests.rs

#[cfg(test)]
mod integration_tests {
    use crate::json_db::collections::manager::CollectionsManager;
    use crate::json_db::storage::{JsonDbConfig, StorageEngine};
    use crate::model_engine::loader::ModelLoader;
    use serde_json::json;
    use std::env;
    use std::fs;
    use std::path::{Path, PathBuf};

    // --- Helper d'initialisation ---
    fn setup_env(space: &str, db: &str) -> (tempfile::TempDir, JsonDbConfig) {
        let tmp_dir = tempfile::tempdir().expect("create temp dir");
        let data_root = tmp_dir.path().to_path_buf();
        let config = JsonDbConfig { data_root };

        let db_root = config.db_root(space, db);
        fs::create_dir_all(&db_root).expect("create db root");

        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let possible_paths = vec![
            manifest_dir.join("../schemas/v1"),
            manifest_dir.join("schemas/v1"),
            PathBuf::from("schemas/v1"),
        ];
        let src_schemas = possible_paths
            .into_iter()
            .find(|p| p.exists())
            .expect("❌ FATAL: 'schemas/v1' introuvable");

        let dest_schemas = config.db_schemas_root(space, db).join("v1");
        if !dest_schemas.exists() {
            fs::create_dir_all(&dest_schemas).unwrap();
        }
        copy_dir_recursive(&src_schemas, &dest_schemas).unwrap();

        let storage = StorageEngine::new(config.clone());
        let mgr = CollectionsManager::new(&storage, space, db);
        mgr.init_db().expect("init_db failed");

        (tmp_dir, config)
    }

    fn copy_dir_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
        if !dst.exists() {
            fs::create_dir_all(dst)?;
        }
        for entry in fs::read_dir(src)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                copy_dir_recursive(&entry.path(), &dst.join(entry.file_name()))?;
            } else {
                fs::copy(entry.path(), dst.join(entry.file_name()))?;
            }
        }
        Ok(())
    }

    #[test]
    fn test_semantic_loading_oa_and_sa() {
        let space = "test_space_model";
        let db = "test_db_model";
        let (_tmp, config) = setup_env(space, db);

        let storage = StorageEngine::new(config.clone());
        let mgr = CollectionsManager::new(&storage, space, db);

        // Insertion OA
        mgr.insert_with_schema(
            "actors",
            json!({
                "@context": { "oa": "https://raise.io/ontology/arcadia/oa#" },
                "@type": "oa:OperationalActor",
                "name": "Opérateur Humain"
            }),
        )
        .expect("insert oa");

        // Insertion SA
        mgr.insert_with_schema(
            "functions",
            json!({
                "@context": { "sa": "https://raise.io/ontology/arcadia/sa#" },
                "@type": "sa:SystemFunction",
                "name": "Calculer Trajectoire"
            }),
        )
        .expect("insert sa");

        let loader = ModelLoader::new_with_manager(mgr);
        let model = loader.load_full_model().expect("load model");

        // Vérifications avec NameType (.as_str())
        assert_eq!(model.oa.actors.len(), 1);
        assert_eq!(model.oa.actors[0].name.as_str(), "Opérateur Humain");
        assert!(model.oa.actors[0].kind.ends_with("OperationalActor"));

        assert_eq!(model.sa.functions.len(), 1);
        assert!(model.sa.functions[0].kind.ends_with("SystemFunction"));
    }

    #[test]
    fn test_loading_data_layer() {
        let space = "test_space_data";
        let db = "test_db_data";
        let (_tmp, config) = setup_env(space, db);

        let storage = StorageEngine::new(config.clone());
        let mgr = CollectionsManager::new(&storage, space, db);

        mgr.insert_with_schema(
            "exchange-items",
            json!({
                "@context": { "data": "https://raise.io/ontology/arcadia/data#" },
                "@type": "data:ExchangeItem",
                "name": "Position GPS",
                "mechanism": "Flow"
            }),
        )
        .expect("insert item");

        let loader = ModelLoader::new_with_manager(mgr);
        let model = loader.load_full_model().expect("load model");

        assert_eq!(model.data.exchange_items.len(), 1);
        assert_eq!(model.data.exchange_items[0].name.as_str(), "Position GPS");
        assert!(model.data.exchange_items[0].kind.ends_with("ExchangeItem"));
    }
}
