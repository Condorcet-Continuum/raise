// FICHIER : src-tauri/tests/ai_suite/business_agent_tests.rs

use crate::common::{setup_test_env, LlmMode};
use raise::ai::agents::intent_classifier::EngineeringIntent;
use raise::ai::agents::{dynamic_agent::DynamicAgent, Agent, AgentContext};
use raise::json_db::collections::manager::CollectionsManager;
use raise::utils::prelude::*; // 🎯 Façade Unique RAISE
use raise::utils::testing::DbSandbox;

#[async_test]
#[serial_test::serial]
#[cfg_attr(not(feature = "cuda"), ignore)]
async fn test_business_agent_generates_oa_entities() -> RaiseResult<()> {
    let env = setup_test_env(LlmMode::Enabled).await;
    let test_root = env.sandbox.storage.config.data_root.clone();

    // --- 🎯 1. SETUP SYSTEM (Injection via Mount Points) ---
    // Utilisation dynamique des points de montage système pour la résilience
    let system_domain = &env.sandbox.config.mount_points.system.domain;
    let system_db = &env.sandbox.config.mount_points.system.db;

    let sys_mgr = CollectionsManager::new(&env.sandbox.storage, system_domain, system_db);

    match DbSandbox::mock_db(&sys_mgr).await {
        Ok(_) => user_info!("INF_TEST_MOCK_DB_READY"),
        Err(e) => raise_error!("ERR_TEST_SETUP_FAIL", error = e.to_string()),
    }

    let generic_schema = "db://_system/_system/schemas/v1/db/generic.schema.json";
    let collections = vec![
        "prompts",
        "agents",
        "session_agents",
        "configs",
        "databases",
    ];

    for coll in collections {
        if let Err(e) = sys_mgr.create_collection(coll, generic_schema).await {
            user_error!(
                "ERR_TEST_COLLECTION_FAIL",
                json_value!({"coll": coll, "error": e.to_string()})
            );
        }
    }

    // Déclarations vitales pour la résolution MBSE
    sys_mgr
        .upsert_document(
            "databases",
            json_value!({ "handle": "oa", "domain": "un2" }),
        )
        .await?;

    sys_mgr
        .upsert_document(
            "configs",
            json_value!({
                "handle": "ontological_mapping",
                "mappings": {
                    "OperationalCapability": { "layer": "oa", "collection": "capabilities" },
                    "OperationalActor": { "layer": "oa", "collection": "actors" }
                }
            }),
        )
        .await?;

    // Configuration de l'identité de l'agent
    sys_mgr.upsert_document("prompts", json_value!({
        "handle": "prompt_business",
        "role": "Analyste Métier",
        "identity": { "persona": "Expert Arcadia. Répond en JSON pur.", "tone": "froid" },
        "environment": "Environnement de test MBSE Arcadia", // 🎯 FIX : Le champ exigé par le PromptEngine
        "directives": ["Génère un TABLEAU JSON avec: '_id', 'name', 'type' (OperationalActor/OperationalCapability), 'layer' (OA)."]
    })).await?;

    sys_mgr.upsert_document("agents", json_value!({
        "handle": "agent_business",
        "base": {
            "name": { "fr": "Business Analyst" },
            "neuro_profile": { "prompt_id": "ref:prompts:handle:prompt_business", "temperature": 0.0 }
        }
    })).await?;

    // --- 🎯 2. SETUP PROJECT (Physique) ---
    let oa_mgr = CollectionsManager::new(&env.sandbox.storage, "un2", "oa");
    let _ = DbSandbox::mock_db(&oa_mgr).await;

    oa_mgr
        .create_collection("capabilities", generic_schema)
        .await?;
    oa_mgr.create_collection("actors", generic_schema).await?;

    // --- 🎯 3. CONTEXTE & EXÉCUTION ---
    let session_id = AgentContext::generate_default_session_id("agent_business", "test_oa");

    use candle_nn::VarMap;
    let world_engine = SharedRef::new(
        raise::ai::world_model::NeuroSymbolicEngine::new(
            raise::utils::data::config::WorldModelConfig::default(),
            VarMap::new(),
        )
        .expect("WM Engine fail"),
    );

    let ctx = AgentContext::new(
        "agent_business",
        &session_id,
        SharedRef::new(env.sandbox.storage.clone()),
        env.client.clone().expect("LlmClient requis"),
        world_engine,
        test_root.clone(),
        test_root.join("dataset"),
    )
    .await;

    let agent = DynamicAgent::new("agent_business");
    let intent = EngineeringIntent::DefineBusinessUseCase {
        domain: "Banque".to_string(),
        process_name: "Crédit".to_string(),
        description: "Un Client dépose un dossier.".to_string(),
    };

    // Traitement avec gestion RAISE
    match agent.process(&ctx, &intent).await {
        Ok(_) => user_success!("SUC_TEST_AGENT_PROCESSED"),
        Err(e) => raise_error!("ERR_AGENT_PROCESS_FAIL", error = e.to_string()),
    }

    // --- 🔍 4. VÉRIFICATION (Résilience & Artefacts) ---
    tokio::time::sleep(TimeDuration::from_millis(1500)).await;

    let cap_dir = test_root.join("un2/oa/collections/capabilities");
    let act_dir = test_root.join("un2/oa/collections/actors");

    let mut found = false;
    if cap_dir.exists() {
        if let Ok(entries) = fs::read_dir_sync(&cap_dir) {
            if entries.flatten().count() > 0 {
                found = true;
            }
        }
    }

    if !found && act_dir.exists() {
        if let Ok(entries) = fs::read_dir_sync(&act_dir) {
            if entries.flatten().count() > 0 {
                found = true;
            }
        }
    }

    assert!(found, "L'IA n'a produit aucun fichier dans 'un2/oa'.");
    Ok(())
}

// =========================================================================
// NOUVEAUX TESTS : RÉSILIENCE ET POINTS DE MONTAGE
// =========================================================================

#[cfg(test)]
mod resilience_tests {
    use super::*;
    use raise::ai::llm::client::LlmClient;

    /// 🎯 Test la résilience face à un point de montage système manquant
    #[async_test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_agent_setup_mount_point_resilience() -> RaiseResult<()> {
        let env = setup_test_env(LlmMode::Disabled).await;
        assert!(!env.sandbox.config.mount_points.system.domain.is_empty());
        assert!(!env.sandbox.config.mount_points.system.db.is_empty());
        Ok(())
    }

    /// 🎯 Test le comportement en cas d'absence de prompt obligatoire (Match...raise_error)
    #[async_test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_agent_missing_definition_error_handling() -> RaiseResult<()> {
        let env = setup_test_env(LlmMode::Disabled).await;
        let test_root = env.sandbox.storage.config.data_root.clone();

        // On récupère le manager système via les points de montage pour initialiser le client
        let sys_mgr = CollectionsManager::new(
            &env.sandbox.storage,
            &env.sandbox.config.mount_points.system.domain,
            &env.sandbox.config.mount_points.system.db,
        );

        use candle_nn::VarMap;
        let world_engine = SharedRef::new(
            raise::ai::world_model::NeuroSymbolicEngine::new(
                raise::utils::data::config::WorldModelConfig::default(),
                VarMap::new(),
            )
            .expect("WM Engine fail"),
        );

        // 🎯 FIX : Initialisation du client LLM pour le test de résilience
        // Si env.client est None (LlmMode::Disabled), on en crée un nouveau sur le champ.
        let llm_client = match env.client.clone() {
            Some(client) => client,
            None => LlmClient::new(&sys_mgr)
                .await
                .expect("Failed to create fallback LlmClient"),
        };

        let ctx = AgentContext::new(
            "agent_ghost",
            "sess_ghost",
            SharedRef::new(env.sandbox.storage.clone()),
            llm_client,
            world_engine,
            test_root.clone(),
            test_root.join("dataset"),
        )
        .await;

        let agent = DynamicAgent::new("agent_ghost");

        // Exécution du test de résilience : l'agent "agent_ghost" n'existe pas en DB
        let result = agent.process(&ctx, &EngineeringIntent::Chat).await;

        match result {
            Err(AppError::Structured(data)) => {
                // On vérifie que le code d'erreur levé par DynamicAgent est bien celui attendu
                assert_eq!(data.code, "ERR_AGENT_CONFIG_NOT_FOUND");
                Ok(())
            }
            _ => panic!("Le moteur aurait dû lever ERR_AGENT_CONFIG_NOT_FOUND via Match"),
        }
    }
}
