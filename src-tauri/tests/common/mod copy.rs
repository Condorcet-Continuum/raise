// FICHIER : src-tauri/tests/common/mod.rs

use raise::ai::llm::client::LlmClient;
use raise::json_db::collections::manager::CollectionsManager;
use raise::json_db::storage::{JsonDbConfig, StorageEngine};
use raise::utils::config::AppConfig;
use raise::utils::{
    async_recursion,
    io::{self, Path, PathBuf},
    prelude::*,
    Once,
};
use raise::utils::config::test_mocks::inject_mock_config;
use std::env;
use tempfile::TempDir;

static INIT: Once = Once::new();

#[allow(dead_code)]
pub struct UnifiedTestEnv {
    pub storage: StorageEngine,
    pub client: LlmClient,
    pub space: String,
    pub db: String,
    pub domain_path: PathBuf,
    pub _tmp_dir: TempDir,
}

pub async fn setup_test_env() -> UnifiedTestEnv {
    INIT.call_once(|| {
        let _ = tracing_subscriber::fmt()
            .with_env_filter("info")
            .with_test_writer()
            .try_init();

        std::env::set_var("RAISE_ENV_MODE", "test");
        AppConfig::init().expect("❌ Échec critique de l'initialisation AppConfig");
    });

    let test_uuid = uuid::Uuid::new_v4().to_string();
    let temp_dir = tempfile::Builder::new()
        .prefix(&format!("raise_test_{}_", test_uuid))
        .tempdir()
        .expect("❌ Impossible de créer le dossier temporaire");

    let domain_path = temp_dir.path().to_path_buf();
    let db_config = JsonDbConfig::new(domain_path.clone());

    let space = "_system".to_string();
    let db = "_system".to_string();

    let db_root = db_config.db_root(&space, &db);
    io::create_dir_all(&db_root).await.expect("create db root");

    // GESTION DES SCHÉMAS
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let possible_paths = vec![
        manifest_dir.join("../schemas/v1"),
        manifest_dir.join("schemas/v1"),
        manifest_dir.join("../../schemas/v1"),
    ];

    let src_schemas = match possible_paths.into_iter().find(|p| p.exists()) {
        Some(path) => path,
        None => {
            eprintln!("⚠️ Schémas introuvables sur le disque. Génération de MOCKS...");
            generate_mock_schemas(&temp_dir.path().join("mock_schemas_src"))
                .await
                .expect("Impossible de générer les schémas mocks")
        }
    };

    let dest_schemas_root = db_config.db_schemas_root(&space, &db).join("v1");
    copy_dir_recursive(&src_schemas, &dest_schemas_root)
        .await
        .expect("copy schemas");

    let ai_models_dir = domain_path.join("_system/ai-assets/models");
    io::create_dir_all(&ai_models_dir)
        .await
        .expect("fail create ai models dir");

    // On crée un fichier vide pour simuler le modèle GGUF
    io::write(&ai_models_dir.join("mock.gguf"), b"dummy model content")
        .await
        .expect("fail to create mock gguf");

    let storage = StorageEngine::new(db_config);
    let mgr = CollectionsManager::new(&storage, &space, &db);
    mgr.init_db().await.expect("init_db failed");

    let app_config = AppConfig::get();

    // 🎯 ÉTAPE CRUCIALE : On récupère le chemin que l'AppConfig utilise réellement.
    // L'erreur montre que c'est /home/zair/raise_domain.
    let ai_domain = app_config
        .get_path("PATH_RAISE_DOMAIN")
        .unwrap_or_else(|| domain_path.clone());

    // On construit le chemin complet du modèle attendu par LlmClient::new()
    let mock_model_file = ai_domain.join("_system/ai-assets/models/mock.gguf");

    // On crée les dossiers parents physiquement
    if let Some(parent) = mock_model_file.parent() {
        std::fs::create_dir_all(parent).expect("Fail to create mock model dir");
    }

    // 🎯 ON CRÉE LE FICHIER PHYSIQUE (satisfait la validation de LlmClient)
    std::fs::write(&mock_model_file, b"dummy content").expect("Fail to write mock.gguf");

    // Maintenant, le client verra le fichier et acceptera de s'initialiser
    let client =
        LlmClient::new().expect("❌ Impossible d'initialiser le LlmClient natif dans les tests");

    UnifiedTestEnv {
        storage,
        client,
        space,
        db,
        domain_path,
        _tmp_dir: temp_dir,
    }
}

async fn generate_mock_schemas(base_path: &Path) -> Result<PathBuf> {
    io::create_dir_all(base_path).await?;

    // 1. ARCADIA DATA
    let arcadia_data = base_path.join("arcadia/data");
    io::create_dir_all(&arcadia_data).await?;
    let exchange_schema = json!({
        "$id": "https://raise.io/schemas/v1/arcadia/data/exchange-item.schema.json",
        "type": "object",
        "properties": { "name": { "type": "string" }, "exchangeMechanism": { "type": "string" } },
        "additionalProperties": true
    });
    io::write_json_atomic(
        &arcadia_data.join("exchange-item.schema.json"),
        &exchange_schema,
    )
    .await?;

    // 2. CONFIGS
    let configs_dir = base_path.join("configs");
    io::create_dir_all(&configs_dir).await?;
    let config_schema = json!({
        "$id": "https://raise.io/schemas/v1/configs/config.schema.json",
        "type": "object",
        "additionalProperties": true
    });
    io::write_json_atomic(&configs_dir.join("config.schema.json"), &config_schema).await?;

    // 3. ACTORS
    let actors_dir = base_path.join("actors");
    io::create_dir_all(&actors_dir).await?;
    let actor_schema = json!({
        "$id": "https://raise.io/schemas/v1/actors/actor.schema.json",
        "type": "object",
        "properties": {
            "handle": { "type": "string" },
            "displayName": { "type": "string" },
            "kind": { "type": "string" },
            "x_age": { "type": "integer" },
            "x_city": { "type": "string" },
            "x_active": { "type": "boolean" },
            "tags": { "type": "array", "items": { "type": "string" } }
        },
        "additionalProperties": true
    });
    io::write_json_atomic(&actors_dir.join("actor.schema.json"), &actor_schema).await?;

    // 4. ARTICLES
    let articles_dir = base_path.join("articles");
    io::create_dir_all(&articles_dir).await?;
    let article_schema = json!({
        "$id": "https://raise.io/schemas/v1/articles/article.schema.json",
        "type": "object",
        "properties": {
            "handle": { "type": "string" },
            "title": { "type": "string" }
        },
        "additionalProperties": true
    });
    io::write_json_atomic(&articles_dir.join("article.schema.json"), &article_schema).await?;

    // 5. WORKUNITS & FINANCE
    let workunits_dir = base_path.join("workunits");
    io::create_dir_all(&workunits_dir).await?;

    let workunit_schema = json!({
        "$id": "https://raise.io/schemas/v1/workunits/workunit.schema.json",
        "type": "object",
        "additionalProperties": true
    });
    io::write_json_atomic(
        &workunits_dir.join("workunit.schema.json"),
        &workunit_schema,
    )
    .await?;

    // ✅ CORRECTION FINALE : "value" au lieu de "source"
    // Conformément à la struct Replace dans ast.rs
    let finance_schema = json!({
        "$id": "https://raise.io/schemas/v1/workunits/finance.schema.json",
        "type": "object",
        "properties": {
            "billing_model": { "type": "string" },
            "revenue_scenarios": { "type": "object" },
            "gross_margin": { "type": "object" },
            "summary": {
                "type": "object",
                "properties": {
                    "net_margin_low": { "type": "number" },
                    "net_margin_mid": { "type": "number" },
                    "mid_is_profitable": { "type": "boolean" },
                    "generated_ref": { "type": "string" }
                }
            }
        },
        "x_rules": [
            {
                "id": "rule-net-low",
                "target": "summary.net_margin_low",
                "expr": { "mul": [ { "var": "revenue_scenarios.low_eur" }, { "var": "gross_margin.low_pct" } ] }
            },
            {
                "id": "rule-net-mid",
                "target": "summary.net_margin_mid",
                "expr": { "mul": [ { "var": "revenue_scenarios.mid_eur" }, { "var": "gross_margin.mid_pct" } ] }
            },
            {
                "id": "rule-profit-check",
                "target": "summary.mid_is_profitable",
                "expr": { "gt": [ { "var": "summary.net_margin_mid" }, { "val": 0 } ] }
            },
            {
                "id": "rule-gen-ref",
                "target": "summary.generated_ref",
                // UTILISATION DU BON NOM DE CHAMP 'value'
                "expr": {
                    "replace": {
                        "value": { "var": "billing_model" }, // ✅ C'était l'erreur (source -> value)
                        "pattern": { "val": "fixed" },
                        "replacement": { "val": "FIN-2025-OK" }
                    }
                }
            }
        ],
        "additionalProperties": true
    });
    io::write_json_atomic(&workunits_dir.join("finance.schema.json"), &finance_schema).await?;

    // 6. DB INDEX
    let db_dir = base_path.join("db");
    io::create_dir_all(&db_dir).await?;
    let index_schema = json!({
        "$id": "https://raise.io/schemas/v1/db/index.schema.json",
        "type": "object",
        "additionalProperties": true
    });
    io::write_json_atomic(&db_dir.join("index.schema.json"), &index_schema).await?;

    Ok(base_path.to_path_buf())
}

#[async_recursion]
async fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<()> {
    if !dst.exists() {
        io::create_dir_all(dst).await?;
    }
    let mut entries = io::read_dir(src).await?;
    while let Some(entry) = entries.next_entry().await? {
        let ty = entry.file_type().await?;
        if ty.is_dir() {
            copy_dir_recursive(&entry.path(), &dst.join(entry.file_name())).await?;
        } else {
            io::copy(entry.path(), dst.join(entry.file_name())).await?;
        }
    }
    Ok(())
}

#[allow(dead_code)]
pub async fn seed_mock_datasets(domain_path: &Path) -> Result<PathBuf> {
    let dataset_dir = domain_path.join("dataset/arcadia/v1/data/exchange-items");
    io::create_dir_all(&dataset_dir)
        .await
        .expect("Create dataset dir");

    let gps_file = dataset_dir.join("position_gps.json");
    let mock_data = json!({ "name": "GPS", "exchangeMechanism": "Flow" });

    io::write_json_atomic(&gps_file, &mock_data)
        .await
        .expect("Write mock data");
    Ok(gps_file)
}
