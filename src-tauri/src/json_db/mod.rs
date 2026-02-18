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
    use crate::utils::config::AppConfig; // ✅ Import nécessaire
    use crate::utils::fs; // ✅ Utilisation de votre fs.rs centralisé
    use crate::utils::prelude::*;
    use crate::utils::Once;
    use std::path::PathBuf;

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

            // On tente d'init la config, mais on ignore l'erreur si déjà init
            let _ = AppConfig::init();
            // Injection de mocks si nécessaire pour les chemins
            crate::utils::config::test_mocks::inject_mock_config();
        });

        let tmp_dir = tempfile::tempdir().expect("create temp dir");
        let data_root = tmp_dir.path().to_path_buf();

        let cfg = JsonDbConfig {
            data_root: data_root.clone(),
        };

        // 1. Création de la structure de base
        let db_root = cfg.db_root(TEST_SPACE, TEST_DB);
        fs::create_dir_all(&db_root).await.expect("create db root");

        // 2. GESTION INTELLIGENTE DES SCHÉMAS (Réel ou Mock)
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let possible_paths = vec![
            manifest_dir.join("../schemas/v1"),
            manifest_dir.join("schemas/v1"),
            PathBuf::from("schemas/v1"),
        ];

        let src_schemas = possible_paths.into_iter().find(|p| p.exists());
        let dest_schemas_root = cfg.db_schemas_root(TEST_SPACE, TEST_DB).join("v1");

        // Création du dossier destination
        fs::create_dir_all(&dest_schemas_root)
            .await
            .expect("create schema dir");

        if let Some(src) = src_schemas {
            // CAS A : Les fichiers réels existent (Dev local) -> On copie
            if let Err(e) = fs::copy_dir_all(&src, &dest_schemas_root).await {
                eprintln!(
                    "⚠️ Warning: Echec copie schémas réels ({}), passage en mode Mock.",
                    e
                );
                generate_mock_schemas(&dest_schemas_root).await;
            }
        } else {
            // CAS B : Environnement isolé (CI/Test) -> On génère des bouchons
            eprintln!(
                "ℹ️ Info: Schémas source introuvables. Génération de schémas Mock pour le test."
            );
            generate_mock_schemas(&dest_schemas_root).await;
        }

        // 3. INITIALISATION DU MOTEUR
        let storage = StorageEngine::new(cfg.clone());
        let mgr = CollectionsManager::new(&storage, TEST_SPACE, TEST_DB);

        // On ignore les erreurs d'init migration si on est en mode mock complet
        let _ = mgr.init_db().await;

        // 4. CRÉATION DES DATASETS MOCKS (Données de test)
        create_mock_dataset(&data_root).await;

        TestEnv {
            cfg,
            storage,
            space: TEST_SPACE.to_string(),
            db: TEST_DB.to_string(),
            tmp_dir,
        }
    }

    /// Génère des schémas minimaux pour permettre au moteur de démarrer sans erreurs
    async fn generate_mock_schemas(root: &PathBuf) {
        // Schéma minimal pour mandates.json (utilisé dans executor.rs)
        let mandate_schema = r#"{
            "$schema": "http://json-schema.org/draft-07/schema#",
            "title": "Mandate",
            "type": "object",
            "properties": { "id": { "type": "string" } },
            "required": ["id"]
        }"#;

        let _ = fs::write(root.join("mandates.json"), mandate_schema).await;

        // Schéma minimal pour l'index système (utilisé par CollectionsManager)
        let index_db_dir = root.join("db");
        let _ = fs::create_dir_all(&index_db_dir).await;

        let index_schema = r#"{
            "$schema": "http://json-schema.org/draft-07/schema#",
            "title": "Database Index",
            "type": "object"
        }"#;
        let _ = fs::write(index_db_dir.join("index.schema.json"), index_schema).await;
    }

    async fn create_mock_dataset(data_root: &PathBuf) {
        let dataset_root = data_root.join("dataset");
        let _ = fs::create_dir_all(&dataset_root).await;

        // Mock Article
        let article_path = dataset_root.join("arcadia/v1/data/articles/article.json");
        if let Some(p) = article_path.parent() {
            let _ = fs::create_dir_all(p).await;
        }

        let mock_article = json!({
            "handle": "mock-handle",
            "displayName": "Mock Article",
            "slug": "mock-slug",
            "title": "Mock Title",
            "status": "draft",
            "authorId": "00000000-0000-0000-0000-000000000000"
        });
        let _ = fs::write_json_atomic(&article_path, &mock_article).await;

        // Mock Exchange Item
        let ex_path = dataset_root.join("arcadia/v1/data/exchange-items/position_gps.json");
        if let Some(p) = ex_path.parent() {
            let _ = fs::create_dir_all(p).await;
        }

        let _ = fs::write_json_atomic(
            &ex_path,
            &json!({ "name": "GPS Position", "mechanism": "Flow" }),
        )
        .await;
    }

    pub async fn ensure_db_exists(cfg: &JsonDbConfig, space: &str, db: &str) {
        let db_path = cfg.db_root(space, db);
        if !db_path.exists() {
            let _ = fs::create_dir_all(&db_path).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::test_utils::*;

    #[tokio::test]
    async fn test_env_initialization() {
        let env = init_test_env().await;
        assert!(env.tmp_dir.path().exists());

        // Test que le fallback (Mock ou Réel) a fonctionné
        // On vérifie simplement que le dossier v1 existe
        let v1_path = env.cfg.db_schemas_root(&env.space, &env.db).join("v1");
        assert!(
            v1_path.exists(),
            "Le dossier schemas/v1 doit exister (Réel ou Mock)"
        );

        // On vérifie qu'un fichier vital existe (mandates.json ou index)
        let has_mandate = v1_path.join("mandates.json").exists();
        let has_index = v1_path.join("db/index.schema.json").exists();

        assert!(
            has_mandate || has_index,
            "Au moins un schéma doit être présent"
        );
    }
}
