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
    use async_recursion::async_recursion;
    use std::env;
    use std::path::{Path, PathBuf};
    use std::sync::Once;
    use tokio::fs; // Utilisation de tokio::fs // Pour la copie récursive async

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

    /// Initialise un environnement de test complet (Async)
    pub async fn init_test_env() -> TestEnv {
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
        fs::create_dir_all(&db_root).await.expect("create db root");

        // 2. COPIE DES SCHÉMAS RÉELS
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));

        let possible_paths = vec![
            manifest_dir.join("../schemas/v1"),
            manifest_dir.join("schemas/v1"),
            PathBuf::from("schemas/v1"),
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
            fs::create_dir_all(&dest_schemas_root)
                .await
                .expect("create schema dir");
        }

        copy_dir_recursive(&src, &dest_schemas_root)
            .await
            .expect("copy schemas failed");

        // 3. INITIALISATION PROPRE DU MOTEUR (Async)
        let storage = StorageEngine::new(cfg.clone());
        let mgr = CollectionsManager::new(&storage, TEST_SPACE, TEST_DB);

        mgr.init_db()
            .await // Migration async
            .expect("Failed to initialize test database via Manager");

        // 4. CRÉATION DES DATASETS MOCKS
        let dataset_root = data_root.join("dataset");
        fs::create_dir_all(&dataset_root).await.unwrap();

        // Mock Article
        let article_rel = "arcadia/v1/data/articles/article.json";
        let article_path = dataset_root.join(article_rel);
        if let Some(p) = article_path.parent() {
            fs::create_dir_all(p).await.unwrap();
        }

        let mock_article = r#"{
            "handle": "mock-handle",
            "displayName": "Mock Article",
            "slug": "mock-slug",
            "title": "Mock Title",
            "status": "draft",
            "authorId": "00000000-0000-0000-0000-000000000000"
        }"#;
        fs::write(&article_path, mock_article).await.unwrap();

        // Mock Exchange Item
        let ex_item_rel = "arcadia/v1/data/exchange-items/position_gps.json";
        let ex_item_path = dataset_root.join(ex_item_rel);
        if let Some(p) = ex_item_path.parent() {
            fs::create_dir_all(p).await.unwrap();
        }
        fs::write(
            &ex_item_path,
            r#"{ "name": "GPS Position", "mechanism": "Flow" }"#,
        )
        .await
        .unwrap();

        TestEnv {
            cfg,
            storage,
            space: TEST_SPACE.to_string(),
            db: TEST_DB.to_string(),
            tmp_dir,
        }
    }

    pub async fn ensure_db_exists(cfg: &JsonDbConfig, space: &str, db: &str) {
        let db_path = cfg.db_root(space, db);
        if !db_path.exists() {
            fs::create_dir_all(&db_path).await.unwrap();
        }
    }

    pub async fn get_dataset_file(cfg: &JsonDbConfig, rel_path: &str) -> PathBuf {
        let root = cfg.data_root.join("dataset");
        let path = root.join(rel_path);

        if let Some(parent) = path.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent)
                    .await
                    .expect("Failed to create dataset parent dir");
            }
        }
        path
    }

    #[async_recursion]
    async fn copy_dir_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
        if !dst.exists() {
            fs::create_dir_all(dst).await?;
        }
        let mut entries = fs::read_dir(src).await?;
        while let Some(entry) = entries.next_entry().await? {
            let ty = entry.file_type().await?;
            let src_path = entry.path();
            let dst_path = dst.join(entry.file_name());

            if ty.is_dir() {
                copy_dir_recursive(&src_path, &dst_path).await?;
            } else if src_path.extension().is_some_and(|e| e == "json") {
                fs::copy(&src_path, &dst_path).await?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::test_utils::*;

    #[tokio::test]
    async fn test_env_initialization() {
        // Vérifie que l'initialisation asynchrone de l'environnement de test fonctionne
        let env = init_test_env().await;
        assert!(env.tmp_dir.path().exists());

        // Vérifie qu'un schéma a bien été copié (le schéma de base index)
        let schema_path = env
            .cfg
            .db_schemas_root(&env.space, &env.db)
            .join("v1/db/index.schema.json");
        assert!(
            schema_path.exists(),
            "Le schéma système devrait être présent"
        );
    }
}
