// FICHIER : src-tauri/tests/common/mod.rs

use raise::ai::llm::client::LlmClient;
use raise::json_db::collections::manager::CollectionsManager;
use raise::json_db::storage::{JsonDbConfig, StorageEngine};
use raise::utils::config::test_mocks::inject_mock_component;

use raise::utils::config::AppConfig;
use raise::utils::{
    async_recursion,
    io::{self, Path, PathBuf, TempDir},
    prelude::*,
    Once,
};

static INIT: Once = Once::new();

#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
pub enum LlmMode {
    Enabled,
    Disabled,
}

#[allow(dead_code)]
pub struct UnifiedTestEnv {
    pub storage: StorageEngine,
    pub client: Option<LlmClient>,
    pub space: String,
    pub db: String,
    pub domain_path: PathBuf,
    pub _tmp_dir: TempDir,
}

pub async fn setup_test_env(llm_mode: LlmMode) -> UnifiedTestEnv {
    INIT.call_once(|| {
        let _ = tracing_subscriber::fmt().with_test_writer().try_init();

        // 🎯 On isole la création du runtime dans un thread natif séparé
        std::thread::spawn(|| {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();

            rt.block_on(async {
                // Utilisation de raise:: car nous sommes en dehors de la crate principale
                raise::utils::config::test_mocks::inject_mock_config().await;
            });
        })
        .join()
        .unwrap(); // On attend la fin du thread et on propage la panique si besoin
    });

    let app_config = AppConfig::get();

    // 🎯 1. ISOLATION : On crée un dossier UNIQUE pour ce test précis
    let test_uuid = uuid::Uuid::new_v4().to_string();
    let temp_dir = tempfile::Builder::new()
        .prefix(&format!("raise_it_{}_", test_uuid))
        .tempdir()
        .expect("❌ Impossible de créer le dossier temporaire");

    let domain_path = temp_dir.path().to_path_buf();

    // 🎯 2. INITIALISATION DB & MANAGER AVANT LE LLM
    let space = "_system".to_string();
    let db = "_system".to_string();
    let db_config = JsonDbConfig::new(domain_path.clone());

    let mock_src = domain_path.join("mock_schemas_src");
    let src_schemas = generate_mock_schemas(&mock_src)
        .await
        .expect("fail mock schemas");

    let dest_schemas = db_config.db_schemas_root(&space, &db).join("v1");
    copy_dir_recursive(&src_schemas, &dest_schemas)
        .await
        .expect("fail copy schemas");

    // Initialisation du stockage isolé
    let storage = StorageEngine::new(db_config);
    let mgr = CollectionsManager::new(&storage, &space, &db);
    mgr.init_db()
        .await
        .expect("❌ Échec de l'initialisation de l'index système");

    // 🎯 3. INJECTION DU MOCK IA EN BASE (Requis pour l'init LLM)
    inject_mock_component(
        &mgr,
        "llm",
        json!({ "rust_tokenizer_file": "tokenizer.json", "rust_model_file": "qwen2.5-1.5b-instruct-q4_k_m.gguf" })
    ).await;

    // 🎯 4. SATISFAIRE LLMCLIENT AVEC LE MANAGER
    let client = match llm_mode {
        LlmMode::Enabled => {
            let global_domain = app_config.get_path("PATH_RAISE_DOMAIN").unwrap();
            let mock_model_file = global_domain.join("_system/ai-assets/models/mock.gguf");
            if let Some(parent) = mock_model_file.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = std::fs::write(&mock_model_file, b"dummy content");

            // Fini le OnceCell ! Le client est désormais lié au bon dossier temporaire.
            let isolated_client = LlmClient::new(&mgr)
                .await
                .expect("❌ Impossible d'initialiser le LlmClient isolé");

            Some(isolated_client)
        }
        LlmMode::Disabled => None,
    };

    UnifiedTestEnv {
        storage,
        client,
        space,
        db,
        domain_path,
        _tmp_dir: temp_dir, // Vital : maintient le dossier vivant pendant le test
    }
}

async fn generate_mock_schemas(base_path: &Path) -> RaiseResult<PathBuf> {
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

    let index_schema_def = json!({
        "$id": "https://raise.io/schemas/v1/db/index.schema.json",
        "type": "object",
        "additionalProperties": true
    });
    io::write_json_atomic(&db_dir.join("index.schema.json"), &index_schema_def).await?;

    // On crée un index qui déclare explicitement les collections
    let index_schema = json!({
        "name": "_system",
        "collections": {
            "agent_sessions": { "schema": "https://raise.io/schemas/v1/configs/config.schema.json" },
            "configuration_items": { "schema": "https://raise.io/schemas/v1/arcadia/data/exchange-item.schema.json" }
        }
    });
    io::write_json_atomic(&db_dir.join("_system.json"), &index_schema).await?;

    Ok(base_path.to_path_buf())
}

#[allow(dead_code)]
#[async_recursion]
async fn copy_dir_recursive(src: &Path, dst: &Path) -> RaiseResult<()> {
    if !dst.exists() {
        io::create_dir_all(dst).await?;
    }
    let mut entries = io::read_dir(src).await?;

    while let Some(entry) = match entries.next_entry().await {
        Ok(e) => e,
        Err(e) => raise_error!(
            "ERR_FS_READ_DIR_ITERATION",
            context = json!({
                "source": src.to_string_lossy(),
                "io_error": e.to_string(),
                "action": "next_entry",
                "hint": "Échec lors de l'itération sur le répertoire. Le dossier a peut-être été déplacé ou supprimé pendant la lecture."
            })
        ),
    } {
        let path = entry.path();
        let file_name = entry.file_name();
        let dest_path = dst.join(&file_name);

        // 1. Détermination du type de fichier
        let ty = match entry.file_type().await {
            Ok(t) => t,
            Err(e) => raise_error!(
                "ERR_FS_METADATA_FETCH",
                context = json!({
                    "path": path.to_string_lossy(),
                    "io_error": e.to_string(),
                    "action": "get_file_type",
                    "hint": "Impossible de lire les métadonnées du fichier pour déterminer s'il s'agit d'un dossier."
                })
            ),
        };

        // 2. Dispatching récursif ou copie directe
        if ty.is_dir() {
            Box::pin(copy_dir_recursive(&path, &dest_path)).await?;
        } else if let Err(e) = io::copy(&path, &dest_path).await {
            raise_error!(
                "ERR_FS_COPY_FAILED",
                context = json!({
                    "from": path.to_string_lossy(),
                    "to": dest_path.to_string_lossy(),
                    "io_error": e.to_string(),
                    "hint": "La copie du fichier a échoué. Vérifiez l'espace disque ou les permissions d'écriture sur la destination."
                })
            );
        }
    }
    Ok(())
}

#[allow(dead_code)]
pub async fn seed_mock_datasets(domain_path: &Path) -> RaiseResult<PathBuf> {
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
