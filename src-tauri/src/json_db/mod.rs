pub mod collections;
pub mod graph;
pub mod indexes;
pub mod jsonld;
pub mod migrations;
pub mod query;
pub mod schema;
pub mod storage;
pub mod transactions;

// ============================================================================
// UTILITAIRES DE TEST (Intégrés & Zéro Dette)
// ============================================================================
#[cfg(test)]
pub mod test_utils {
    use crate::json_db::collections::manager::CollectionsManager;
    use crate::json_db::storage::JsonDbConfig;
    use crate::utils::data::config::{BOOTSTRAP_DB, BOOTSTRAP_DOMAIN};
    use crate::utils::prelude::*;
    use crate::utils::testing::DbSandbox;

    static TEST_LOGGER_INIT: StaticCell<()> = StaticCell::new();

    pub const TEST_SPACE: &str = "test_space";
    pub const TEST_DB: &str = "test_db";

    /// Environnement de test isolé avec Sandbox intégrée.
    pub struct TestEnv {
        pub sandbox: DbSandbox,
        pub cfg: JsonDbConfig,
        pub space: String,
        pub db: String,
    }

    /// Initialise un environnement de test complet, propre et standardisé.
    pub async fn init_test_env() -> RaiseResult<TestEnv> {
        // 1. Initialisation unique du Logger
        TEST_LOGGER_INIT.get_or_init(|| {
            let _ = tracing_subscriber::fmt()
                .with_env_filter("info")
                .with_test_writer()
                .try_init();
            ()
        });

        // 2. 🎯 LA SANDBOX : Gère l'isolation totale et le schéma maître (_system)
        let sandbox = DbSandbox::new().await?;
        let data_root = sandbox.config.get_path("PATH_RAISE_DOMAIN").unwrap();
        let cfg = JsonDbConfig::new(data_root.clone());

        // 3. Initialisation du manager de test sur la partition spécifique
        let mgr = CollectionsManager::new(&sandbox.storage, TEST_SPACE, TEST_DB);
        DbSandbox::mock_db(&mgr).await?;

        // 4. Création des datasets de test (Données métiers)
        create_mock_dataset(&sandbox, &data_root).await?;

        Ok(TestEnv {
            sandbox,
            cfg,
            space: TEST_SPACE.to_string(),
            db: TEST_DB.to_string(),
        })
    }

    /// Peuple la sandbox avec des données simulées alignées sur les ontologies RAISE.
    async fn create_mock_dataset(sandbox: &DbSandbox, data_root: &PathBuf) -> RaiseResult<()> {
        let dataset_root = data_root.join("dataset");

        // 1. MOCK DATASET (Articles & MBSE)
        let article_path = dataset_root.join("arcadia/v1/data/articles/article.json");
        fs::ensure_dir_async(article_path.parent().unwrap()).await?;

        let mock_article = json_value!({
            "_id": "mock-article-001",
            "handle": "mock-handle",
            "title": "MBSE and AI Integration",
            "status": "draft"
        });
        fs::write_json_atomic_async(&article_path, &mock_article).await?;

        // 2. 🎯 MOCK SYSTEM (Collections d'infrastructure)
        // On utilise les constantes BOOTSTRAP_DOMAIN/DB pour éviter les dossiers fantômes
        let manager = CollectionsManager::new(&sandbox.storage, BOOTSTRAP_DOMAIN, BOOTSTRAP_DB);

        // Injection d'une dApp de test via le manager (plus propre que fs::write)
        let generic_schema = format!(
            "db://{}/{}/schemas/v1/db/generic.schema.json",
            BOOTSTRAP_DOMAIN, BOOTSTRAP_DB
        );
        manager.create_collection("dapps", &generic_schema).await?;

        manager
            .insert_raw(
                "dapps",
                &json_value!({
                    "_id": "raise-core",
                    "handle": "raise_core",
                    "plugin_config": { "rust_package_name": "raise_core" }
                }),
            )
            .await?;

        Ok(())
    }
}
