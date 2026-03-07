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
// ============================================================================
#[cfg(test)]
pub mod test_utils {
    use crate::json_db::collections::manager::CollectionsManager;
    use crate::json_db::storage::JsonDbConfig;
    use crate::utils::io;
    use crate::utils::mock::DbSandbox;
    use crate::utils::Once;
    use std::path::PathBuf;

    static INIT: Once = Once::new();

    pub const TEST_SPACE: &str = "test_space";
    pub const TEST_DB: &str = "test_db";

    /// Notre environnement de test intègre maintenant la Sandbox comme moteur principal
    pub struct TestEnv {
        pub sandbox: DbSandbox, // 🎯 Encapsulation parfaite du moteur et du dossier
        pub cfg: JsonDbConfig,
        pub space: String,
        pub db: String,
    }

    /// Initialise un environnement de test complet et isolé
    pub async fn init_test_env() -> TestEnv {
        // 1. Initialisation unique du Logger (plus besoin de thread complexe !)
        INIT.call_once(|| {
            let _ = tracing_subscriber::fmt()
                .with_env_filter("info")
                .with_test_writer()
                .try_init();
        });

        // 2. 🎯 LA MAGIE : La Sandbox gère l'isolation, la DB, et le schéma maître (_system)
        let sandbox = DbSandbox::new().await;

        let data_root = sandbox.config.get_path("PATH_RAISE_DOMAIN").unwrap();
        let cfg = JsonDbConfig::new(data_root.clone());

        // 3. Initialisation du namespace de test spécifique (TEST_SPACE / TEST_DB)
        let mgr = CollectionsManager::new(&sandbox.storage, TEST_SPACE, TEST_DB);
        mgr.init_db()
            .await
            .expect("Erreur init_db pour l'espace de test");

        // 4. Création des datasets
        create_mock_dataset(&data_root).await;

        TestEnv {
            sandbox,
            cfg,
            space: TEST_SPACE.to_string(),
            db: TEST_DB.to_string(),
        }
    }

    async fn create_mock_dataset(data_root: &PathBuf) {
        let dataset_root = data_root.join("dataset");
        let _ = io::create_dir_all(&dataset_root).await;

        // Mock Article
        let article_path = dataset_root.join("arcadia/v1/data/articles/article.json");
        if let Some(p) = article_path.parent() {
            let _ = io::create_dir_all(p).await;
        }

        // ✅ CORRECTION : snake_case + _id
        let mock_article = serde_json::json!({
            "_id": "mock-article-001",
            "handle": "mock-handle",
            "display_name": "Mock Article",
            "slug": "mock-slug",
            "title": "Mock Title",
            "status": "draft",
            "author_id": "00000000-0000-0000-0000-000000000000"
        });
        let _ = io::write_json_atomic(&article_path, &mock_article).await;

        // Mock Exchange Item
        let ex_path = dataset_root.join("arcadia/v1/data/exchange-items/position_gps.json");
        if let Some(p) = ex_path.parent() {
            let _ = io::create_dir_all(p).await;
        }

        // ✅ CORRECTION : Ajout de _id
        let _ = io::write_json_atomic(
            &ex_path,
            &serde_json::json!({ "_id": "mock-gps-001", "name": "GPS Position", "mechanism": "Flow" }),
        )
        .await;

        let system_collections = data_root.join("_system/_system/collections");

        let dapp_id = "mock-dapp-id";
        let dapp_path = system_collections.join("dapps");
        let _ = io::create_dir_all(&dapp_path).await;

        // ✅ CORRECTION : _id + snake_case pour plugin_config
        let _ = io::write_json_atomic(
            &dapp_path.join(format!("{}.json", dapp_id)),
            &serde_json::json!({
                "_id": dapp_id,
                "handle": "raise-core",
                "name": "raise_core",
                "plugin_config": { "rust_package_name": "raise_core" }
            }),
        )
        .await;

        let service_id = "mock-ai-service-id";
        let services_path = system_collections.join(format!("dapps/{}/services", dapp_id));
        let _ = io::create_dir_all(&services_path).await;

        // ✅ CORRECTION : _id
        let _ = io::write_json_atomic(
            &services_path.join(format!("{}.json", service_id)),
            &serde_json::json!({
                "_id": service_id,
                "identity": { "service_id": "AI", "status": "enabled" }
            }),
        )
        .await;

        let components_path = system_collections.join(format!(
            "dapps/{}/services/{}/components",
            dapp_id, service_id
        ));
        let _ = io::create_dir_all(&components_path).await;

        // ✅ CORRECTION : _id
        let _ = io::write_json_atomic(
            &components_path.join("mock-llm-comp.json"),
            &serde_json::json!({
                "_id": "mock-llm-comp",
                "identity": { "component_id": "llm", "version": "1.0.0" },
                "settings": {
                    "provider": "candle_native",
                    "model_name": "llama3-1b-test",
                    "rust_repo_id": "Qwen/Qwen2.5-1.5B-Instruct-GGUF"
                }
            }),
        )
        .await;

        // ✅ CORRECTION : _id
        let _ = io::write_json_atomic(
            &components_path.join("mock-mem-comp.json"),
            &serde_json::json!({
                "_id": "mock-mem-comp",
                "identity": { "component_id": "memory", "version": "1.0.0" },
                "settings": {
                    "provider": "candle_embeddings",
                    "model_name": "minilm-test"
                }
            }),
        )
        .await;
    }

    pub async fn ensure_db_exists(cfg: &JsonDbConfig, space: &str, db: &str) {
        let db_path = cfg.db_root(space, db);
        if !db_path.exists() {
            let _ = io::create_dir_all(&db_path).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::test_utils::*;

    #[tokio::test]
    async fn test_env_initialization() {
        let env = init_test_env().await;

        // 🎯 L'accès au chemin temporaire se fait désormais via la config de la sandbox
        let data_root = env.sandbox.config.get_path("PATH_RAISE_DOMAIN").unwrap();
        assert!(data_root.exists());

        // Test que l'injection centralisée a fonctionné
        let sys_schemas_dir = env.cfg.db_schemas_root("_system", "_system").join("v1");
        let has_index = sys_schemas_dir.join("db/index.schema.json").exists();

        assert!(
            has_index,
            "L'index.schema.json maître doit être présent dans le dossier temporaire"
        );
    }
}
