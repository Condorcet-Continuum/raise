// FICHIER : src-tauri/tests/common/mod.rs

use raise::ai::llm::client::LlmClient;
use raise::json_db::collections::manager::CollectionsManager;
use raise::utils::prelude::*;
use raise::utils::testing::{inject_mock_component, AgentDbSandbox}; // 🎯 On passe à AgentDbSandbox

static INIT: InitGuard = InitGuard::new();

#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
pub enum LlmMode {
    Enabled,
    Disabled,
}

#[allow(dead_code)]
pub struct UnifiedTestEnv {
    pub sandbox: AgentDbSandbox, // 🎯 UPGRADE : La sandbox intègre désormais toute l'IA
    pub client: Option<LlmClient>,
    pub space: String,
    pub db: String,
}

// 🧹 SUPPRESSION TOTALE de `get_test_wm_config()` !

/// Initialise un environnement de test robuste, isolé et aligné sur le Registre de Ressources.
pub async fn setup_test_env(llm_mode: LlmMode) -> RaiseResult<UnifiedTestEnv> {
    INIT.call_once(|| {
        let _ = tracing_subscriber::fmt().with_test_writer().try_init();
    });

    // 1. ISOLATION : Création de la Sandbox (L'IA est injectée automatiquement ici !)
    let sandbox = AgentDbSandbox::new().await?;
    let config = &sandbox.config;

    // 2. RÉSOLUTION DES POINTS DE MONTAGE VIA LA FAÇADE
    let (sys_domain, sys_db, _) = config.resolve_system_uri(None, "bootstrap");

    // 4. PRÉPARATION DU MANAGER ET DES SCHÉMAS PHYSIQUES
    // (L'AgentDbSandbox a déjà mocké la DB système, on récupère juste le manager)
    let mgr = CollectionsManager::new(&sandbox.db, &sys_domain, &sys_db);

    // =========================================================================
    // 🎯 INITIALISATION DES COLLECTIONS MÉTIER
    // =========================================================================
    let generic_schema_uri = format!(
        "db://{}/{}/schemas/v1/db/generic.schema.json",
        sys_domain, sys_db
    );

    // A. Collections Système spécifiques aux tests d'intégration
    let system_collections = vec![
        "session_agents",
        "prompts",
        "agents",
        "configs",
        "service_configs",
    ];
    for coll in system_collections {
        let _ = mgr.create_collection(coll, &generic_schema_uri).await;
    }

    // B. Couches MBSE Arcadia (Partition Métier 'un2')
    let layers = vec![
        ("oa", vec!["capabilities", "actors"]),
        ("data", vec!["classes", "types"]),
        ("sa", vec!["functions"]),
        ("la", vec!["components", "functions"]),
        ("pa", vec!["physical_nodes"]),
        ("transverse", vec!["requirements", "test_procedures"]),
        ("epbs", vec!["configuration_items"]),
    ];

    for (db_name, collections) in layers {
        let layer_mgr = CollectionsManager::new(&sandbox.db, "un2", db_name);
        let _ = raise::utils::testing::DbSandbox::mock_db(&layer_mgr).await;
        for coll in collections {
            let _ = layer_mgr.create_collection(coll, &generic_schema_uri).await;
        }
    }

    // =========================================================================
    // 5. INJECTION DE LA CONFIGURATION (Data-Driven)
    // =========================================================================

    // 🎯 TABLE DE ROUTAGE ONTOLOGIQUE
    mgr.upsert_document(
        "configs",
        json_value!({
            "_id": "ref:configs:handle:ontological_mapping",
            "handle": "ontological_mapping",
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
                "DataType": { "layer": "data", "collection": "types" },
                "Function": { "layer": "sa", "collection": "functions" },
                "LogicalFunction": { "layer": "la", "collection": "functions" },
                "LogicalComponent": { "layer": "la", "collection": "components" },
                "Requirement": { "layer": "transverse", "collection": "requirements" }
            }
        }),
    )
    .await?;

    // 🧹 SUPPRESSION MAGISTRALE : Les DEUX injections manuelles du "llm" ont disparu !

    // On conserve uniquement l'injection de la config "ai_agents" (spécifique aux workflows d'intégration)
    inject_mock_component(
        &mgr,
        "ai_agents",
        json_value!({
            "target_domain": "un2",
            "system_domain": sys_domain,
            "system_db": sys_db
        }),
    )
    .await?;

    // =========================================================================
    // 6. INITIALISATION LLM
    // =========================================================================
    let client = match llm_mode {
        LlmMode::Enabled => Some(LlmClient::new(&mgr).await?),
        LlmMode::Disabled => None,
    };

    Ok(UnifiedTestEnv {
        sandbox,
        client,
        space: sys_domain,
        db: sys_db,
    })
}

/// Génère des jeux de données mock.
#[allow(dead_code)]
pub async fn seed_mock_datasets(domain_path: &Path) -> RaiseResult<PathBuf> {
    let dataset_dir = domain_path.join("dataset/arcadia/v1/data/exchange-items");
    fs::create_dir_all_async(&dataset_dir).await?;

    let gps_file = dataset_dir.join("position_gps.json");
    let mock_data = json_value!({ "name": "GPS", "exchangeMechanism": "Flow" });

    fs::write_json_atomic_async(&gps_file, &mock_data).await?;
    Ok(gps_file)
}
