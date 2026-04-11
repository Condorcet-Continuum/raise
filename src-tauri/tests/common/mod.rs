// FICHIER : src-tauri/tests/common/mod.rs

use raise::ai::llm::client::LlmClient;
use raise::json_db::collections::manager::CollectionsManager;
// 🎯 Utilisation de la façade testing pour l'isolation totale
use raise::utils::prelude::*;
use raise::utils::testing::{inject_collection_schema, inject_mock_component, DbSandbox}; // 🎯 Façade Unique RAISE

static INIT: InitGuard = InitGuard::new();

#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
pub enum LlmMode {
    Enabled,
    Disabled,
}

#[allow(dead_code)]
pub struct UnifiedTestEnv {
    pub sandbox: DbSandbox,
    pub client: Option<LlmClient>,
    pub space: String,
    pub db: String,
}

/// Initialise un environnement de test robuste et résilien
pub async fn setup_test_env(llm_mode: LlmMode) -> UnifiedTestEnv {
    INIT.call_once(|| {
        let _ = tracing_subscriber::fmt().with_test_writer().try_init();
    });

    // 1. ISOLATION : Création de la Sandbox (Config, Storage, TempDir)
    let sandbox = DbSandbox::new().await;

    // 🎯 RÉSILIENCE MOUNT POINTS : Utilisation dynamique de la config sandbox
    let system_domain = sandbox.config.mount_points.system.domain.clone();
    let system_db = sandbox.config.mount_points.system.db.clone();
    let domain_path = match sandbox.config.get_path("PATH_RAISE_DOMAIN") {
        Some(path) => path,
        None => panic!("❌ PATH_RAISE_DOMAIN manquant dans la config sandbox"),
    };

    // Création du marqueur d'environnement de test pour les outils
    if let Err(e) = std::fs::write(domain_path.join(".is_test_env"), "1") {
        panic!("❌ Impossible de créer le marqueur de test : {}", e);
    }

    // 2. INITIALISATION DU SYSTÈME SÉMANTIQUE MOCKÉ
    raise::json_db::jsonld::VocabularyRegistry::init_mock_for_tests();

    // 3. PRÉPARATION DES SCHÉMAS PHYSIQUES
    inject_custom_test_schemas(&domain_path).await;

    // 4. INITIALISATION DU MANAGER SYSTÈME
    let mgr = CollectionsManager::new(&sandbox.storage, &system_domain, &system_db);

    match DbSandbox::mock_db(&mgr).await {
        Ok(_) => user_success!("SUC_TEST_DB_READY"),
        Err(e) => panic!("❌ Échec initialisation index système : {}", e),
    }
    inject_custom_test_schemas(&domain_path).await;
    // =========================================================================
    // 🎯 INITIALISATION DES COLLECTIONS (Résilience & Isolation)
    // =========================================================================
    let generic_schema = "db://_system/_system/schemas/v1/db/generic.schema.json";

    // A. Collections Système
    let system_collections = vec!["session_agents", "prompts", "agents", "configs"];
    for coll in system_collections {
        if let Err(e) = mgr.create_collection(coll, generic_schema).await {
            user_error!(
                "ERR_TEST_COLLECTION_FAIL",
                json_value!({"coll": coll, "error": e.to_string()})
            );
        }
    }

    // B. Couches MBSE Arcadia (Partition 'un2')
    let layers = vec![
        ("oa", vec!["capabilities", "actors"]),
        ("data", vec!["classes", "types"]),
        ("sa", vec!["functions"]),
        ("la", vec!["components", "functions"]),
        ("pa", vec!["physical_nodes"]),
        (
            "transverse",
            vec!["requirements", "test_procedures", "test_campaigns"],
        ),
        ("epbs", vec!["configuration_items"]),
    ];

    for (db_name, collections) in layers {
        let layer_mgr = CollectionsManager::new(&sandbox.storage, "un2", db_name);
        let _ = DbSandbox::mock_db(&layer_mgr).await;
        for coll in collections {
            let _ = layer_mgr.create_collection(coll, generic_schema).await;
        }
    }

    // 5. INJECTION DE LA CONFIGURATION (Data-Driven)
    inject_mock_component(
        &mgr,
        "ai_agents",
        json_value!({
            "target_domain": "un2",
            "system_domain": system_domain,
            "system_db": system_db
        }),
    )
    .await;

    // Mapping Ontologique Standard Arcadia
    let _ = mgr
        .upsert_document(
            "configs",
            json_value!({
                "_id": "ref:configs:handle:ontological_mapping",
                "handle": "ontological_mapping",
                // 🎯 FIX : Ajout des search_spaces requis par le ModelLoader
                "search_spaces": [
                    { "layer": "oa", "collection": "capabilities" },
                    { "layer": "oa", "collection": "actors" },
                    { "layer": "data", "collection": "classes" },
                    { "layer": "data", "collection": "types" },
                    { "layer": "sa", "collection": "functions" },
                    { "layer": "la", "collection": "components" },
                    { "layer": "la", "collection": "functions" },
                    { "layer": "pa", "collection": "physical_nodes" },
                    { "layer": "transverse", "collection": "requirements" }
                ],
                "mappings": {
                    "Class": { "layer": "data", "collection": "classes" },
                    "Function": { "layer": "sa", "collection": "functions" },
                    "LogicalFunction": { "layer": "la", "collection": "functions" },
                    "LogicalComponent": { "layer": "la", "collection": "components" },
                    "Requirement": { "layer": "transverse", "collection": "requirements" }
                }
            }),
        )
        .await;

    // 6. INITIALISATION LLM
    inject_mock_component(&mgr, "llm", json_value!({})).await;

    let client = match llm_mode {
        LlmMode::Enabled => {
            let mock_model_file = domain_path.join("_system/ai-assets/models/mock.gguf");
            let _ = fs::ensure_dir_sync(mock_model_file.parent().unwrap());
            let _ = fs::write_sync(&mock_model_file, b"dummy");
            LlmClient::new(&mgr).await.ok()
        }
        LlmMode::Disabled => None,
    };

    UnifiedTestEnv {
        sandbox,
        client,
        space: system_domain,
        db: system_db,
    }
}

/// Injection des schémas JSON pour la validation des tests
/// Injection des schémas JSON pour la validation des tests
async fn inject_custom_test_schemas(domain_root: &Path) {
    let schemas = vec![
        (
            "configuration_items",
            r#"{ "type": "object", "properties": { "name": { "type": "string" } } }"#,
        ),
        (
            "actors",
            r#"{ "type": "object", "properties": { "handle": { "type": "string" } } }"#,
        ),
        (
            "articles",
            r#"{ "type": "object", "properties": { "title": { "type": "string" } } }"#,
        ),
        (
            "finance",
            r#"{
                "type": "object",
                "x_rules": [
                    { 
                        "_id": "rule_net_margin_low",
                        "target": "summary.net_margin_low", 
                        "expr": { "mul": [ { "var": "revenue_scenarios.low_eur" }, { "var": "gross_margin.low_pct" } ] }
                    },
                    { 
                        "_id": "rule_net_margin_mid",
                        "target": "summary.net_margin_mid", 
                        "expr": { "mul": [ { "var": "revenue_scenarios.mid_eur" }, { "var": "gross_margin.mid_pct" } ] }
                    },
                    { 
                        "_id": "rule_mid_profitable",
                        "target": "summary.mid_is_profitable", 
                        "expr": { "gt": [ { "var": "summary.net_margin_mid" }, { "val": 0 } ] }
                    },
                    { 
                        "_id": "rule_gen_ref",
                        "target": "summary.generated_ref", 
                        "expr": {
                            "replace": {
                                "value": { "var": "billing_model" },
                                "pattern": { "val": "fixed" },
                                "replacement": { "val": "FIN-2025-OK" }
                            }
                        }
                    }
                ]
            }"#,
        ),
    ];

    for (name, content) in schemas {
        inject_collection_schema(domain_root, name, content).await;
    }
}

/// Génère des jeux de données mock pour les tests de RAG/Traceability
#[allow(dead_code)]
pub async fn seed_mock_datasets(domain_path: &Path) -> RaiseResult<PathBuf> {
    let dataset_dir = domain_path.join("dataset/arcadia/v1/data/exchange-items");
    match fs::create_dir_all_async(&dataset_dir).await {
        Ok(_) => {
            let gps_file = dataset_dir.join("position_gps.json");
            let mock_data = json_value!({ "name": "GPS", "exchangeMechanism": "Flow" });
            match fs::write_json_atomic_async(&gps_file, &mock_data).await {
                Ok(_) => Ok(gps_file),
                Err(e) => raise_error!("ERR_TEST_SEED_FAIL", error = e.to_string()),
            }
        }
        Err(e) => raise_error!("ERR_TEST_DIR_FAIL", error = e.to_string()),
    }
}

// =========================================================================
// TESTS DE RÉSILIENCE DU COMMON (Zéro Dette)
// =========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[async_test]
    async fn test_env_isolation_resilience() -> RaiseResult<()> {
        let env1 = setup_test_env(LlmMode::Disabled).await;
        let env2 = setup_test_env(LlmMode::Disabled).await;

        // On vérifie que les chemins temporaires sont distincts (Isolation physique)
        assert_ne!(
            env1.sandbox.config.get_path("PATH_RAISE_DOMAIN").unwrap(),
            env2.sandbox.config.get_path("PATH_RAISE_DOMAIN").unwrap()
        );
        Ok(())
    }

    #[async_test]
    async fn test_mount_point_config_integrity() -> RaiseResult<()> {
        let env = setup_test_env(LlmMode::Disabled).await;
        // Vérifie que les mount points système sont bien injectés dans l'env de test
        assert!(!env.sandbox.config.mount_points.system.domain.is_empty());
        Ok(())
    }
}
