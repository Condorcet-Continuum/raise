// FICHIER : src-tauri/src/json_db/mod.rs

pub mod collections;
pub mod indexes;
pub mod jsonld;
pub mod migrations;
pub mod query;
pub mod schema;
pub mod storage;
pub mod transactions;

// ============================================================================
// UTILITAIRES DE TEST (Intégrés)
// Ce module n'est compilé que lors de l'exécution des tests (cargo test)
// ============================================================================
#[cfg(test)]
pub mod test_utils {
    use crate::json_db::collections::manager::CollectionsManager;
    use crate::json_db::storage::{JsonDbConfig, StorageEngine};
    use std::env;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::Once;

    static INIT: Once = Once::new();

    pub const TEST_SPACE: &str = "test_space";
    pub const TEST_DB: &str = "test_db";

    pub struct TestEnv {
        pub cfg: JsonDbConfig,
        pub storage: StorageEngine,
        pub space: String,
        pub db: String,
        pub tmp_dir: tempfile::TempDir,
    }

    pub fn init_test_env() -> TestEnv {
        // Initialisation du logger une seule fois
        INIT.call_once(|| {
            let _ = tracing_subscriber::fmt()
                .with_env_filter("info")
                .with_test_writer()
                .try_init();
        });

        let tmp_dir = tempfile::tempdir().expect("create temp dir");
        let data_root = tmp_dir.path().to_path_buf();

        let cfg = JsonDbConfig {
            data_root: data_root.clone(),
        };

        // 1. Création de la structure de base
        let db_root = cfg.db_root(TEST_SPACE, TEST_DB);
        fs::create_dir_all(&db_root).expect("create db root");

        // 2. COPIE DES SCHÉMAS RÉELS
        // On doit absolument trouver les schémas, sinon le test ne vaut rien.
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")); // Point vers src-tauri/

        // Stratégie de recherche robuste pour les schémas
        let possible_paths = vec![
            manifest_dir.join("../schemas/v1"), // Cas standard développement (workspace root)
            manifest_dir.join("schemas/v1"),    // Cas si copié dedans
            PathBuf::from("schemas/v1"),        // Cas exécution depuis root workspace
        ];

        let src_schemas = possible_paths.into_iter().find(|p| p.exists());

        let src = src_schemas.unwrap_or_else(|| {
            panic!(
                "❌ FATAL: Impossible de trouver le dossier 'schemas/v1' pour les tests.\nRecherché dans : {:?}",
                vec![
                    manifest_dir.join("../schemas/v1"),
                    manifest_dir.join("schemas/v1"),
                    PathBuf::from("schemas/v1"),
                ]
            );
        });

        let dest_schemas_root = cfg.db_schemas_root(TEST_SPACE, TEST_DB).join("v1");
        if !dest_schemas_root.exists() {
            fs::create_dir_all(&dest_schemas_root).expect("create schema dir");
        }

        copy_dir_recursive(&src, &dest_schemas_root).expect("copy schemas failed");

        // 3. INITIALISATION PROPRE DU MOTEUR
        let storage = StorageEngine::new(cfg.clone());
        let mgr = CollectionsManager::new(&storage, TEST_SPACE, TEST_DB);

        // Si cette étape échoue (ex: schéma index invalide), on le saura tout de suite
        mgr.init_db()
            .expect("Failed to initialize test database via Manager");

        // 4. CRÉATION DES DATASETS MOCKS (Pour les tests d'import)
        let dataset_root = data_root.join("dataset");
        fs::create_dir_all(&dataset_root).unwrap();

        // Mock Article
        let article_rel = "arcadia/v1/data/articles/article.json";
        let article_path = dataset_root.join(article_rel);
        if let Some(p) = article_path.parent() {
            fs::create_dir_all(p).unwrap();
        }

        let mock_article = r#"{
            "handle": "mock-handle",
            "displayName": "Mock Article",
            "slug": "mock-slug",
            "title": "Mock Title",
            "status": "draft",
            "authorId": "00000000-0000-0000-0000-000000000000"
        }"#;
        fs::write(&article_path, mock_article).unwrap();

        // Mock Exchange Item
        let ex_item_rel = "arcadia/v1/data/exchange-items/position_gps.json";
        let ex_item_path = dataset_root.join(ex_item_rel);
        if let Some(p) = ex_item_path.parent() {
            fs::create_dir_all(p).unwrap();
        }
        fs::write(
            &ex_item_path,
            r#"{ "name": "GPS Position", "mechanism": "Flow" }"#,
        )
        .unwrap();

        TestEnv {
            cfg,
            storage,
            space: TEST_SPACE.to_string(),
            db: TEST_DB.to_string(),
            tmp_dir,
        }
    }

    pub fn ensure_db_exists(cfg: &JsonDbConfig, space: &str, db: &str) {
        let db_path = cfg.db_root(space, db);
        if !db_path.exists() {
            std::fs::create_dir_all(&db_path).unwrap();
        }
    }

    pub fn get_dataset_file(cfg: &JsonDbConfig, rel_path: &str) -> PathBuf {
        let root = cfg.data_root.join("dataset");
        let path = root.join(rel_path);

        if let Some(parent) = path.parent() {
            if !parent.exists() {
                std::fs::create_dir_all(parent).expect("Failed to create dataset parent dir");
            }
        }
        path
    }

    fn copy_dir_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
        if !dst.exists() {
            fs::create_dir_all(dst)?;
        }
        for entry in fs::read_dir(src)? {
            let entry = entry?;
            let ty = entry.file_type()?;
            let src_path = entry.path();
            let dst_path = dst.join(entry.file_name());

            if ty.is_dir() {
                copy_dir_recursive(&src_path, &dst_path)?;
            } else if src_path.extension().is_some_and(|e| e == "json") {
                fs::copy(&src_path, &dst_path)?;
            }
        }
        Ok(())
    }
}
