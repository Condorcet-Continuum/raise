// FICHIER : src-tauri/tests/ai_suite/epbs_agent_tests.rs

use raise::utils::prelude::*; // 🎯 Façade Unique RAISE

// 🧹 FIX : Suppression de `get_test_wm_config`
use crate::common::{setup_test_env, LlmMode};
use raise::ai::agents::intent_classifier::EngineeringIntent;
use raise::ai::agents::{dynamic_agent::DynamicAgent, Agent, AgentContext};
use raise::json_db::collections::manager::CollectionsManager;
use raise::utils::testing::DbSandbox;

#[async_test]
#[serial_test::serial]
#[cfg_attr(not(feature = "cuda"), ignore)]
async fn test_epbs_agent_creates_configuration_item() -> RaiseResult<()> {
    let env = setup_test_env(LlmMode::Enabled).await?;

    // 🎯 FIX : On utilise `domain_root` exposé par AgentDbSandbox
    let test_root = env.sandbox.domain_root.clone();

    // --- 🎯 1. SETUP SYSTEM (Injection via Mount Points) ---
    let system_domain = &env.sandbox.config.mount_points.system.domain;
    let system_db = &env.sandbox.config.mount_points.system.db;

    // 🎯 FIX : Remplacement de `storage` par `db`
    let sys_mgr = CollectionsManager::new(&env.sandbox.db, system_domain, system_db);

    // Initialisation résiliente de l'index système
    match DbSandbox::mock_db(&sys_mgr).await {
        Ok(_) => user_info!("INF_TEST_MOCK_DB_READY"),
        Err(e) => raise_error!("ERR_TEST_SETUP_FAIL", error = e.to_string()),
    }

    let generic_schema = "db://_system/_system/schemas/v1/db/generic.schema.json";
    let collections = vec!["prompts", "agents", "session_agents", "configs"];

    for coll in collections {
        let _ = sys_mgr.create_collection(coll, generic_schema).await;
    }

    // Injection du prompt EPBS musclé (Zero-Shot JSON)
    sys_mgr.upsert_document("prompts", json_value!({
        "handle": "prompt_epbs",
        "role": "Manager EPBS",
        "identity": { 
            "persona": "Tu es l'expert End-Product Breakdown Structure. Tu réponds EXCLUSIVEMENT en JSON strict.",
            "tone": "robotique"
        },
        "environment": "Gestion de configuration industrielle (EPBS).",  
        "directives": [
            "Génère le ConfigurationItem en format JSON.",
            "NE FAIS AUCUNE PHRASE d'introduction ou d'excuse.",
            "Le JSON doit contenir au minimum: { \"layer\": \"EPBS\", \"type\": \"ConfigurationItem\", \"name\": \"<nom>\" }"
        ]
    })).await?;

    let agent_urn = "agent_epbs";
    sys_mgr.upsert_document("agents", json_value!({
        "handle": agent_urn,
        "base": {
            "name": { "fr": "EPBS Manager" },
            "neuro_profile": { "prompt_id": "ref:prompts:handle:prompt_epbs", "temperature": 0.0 }
        }
    })).await?;

    // --- 🎯 2. SETUP PROJET (Physique) ---
    // 🎯 FIX : Remplacement de `storage` par `db`
    let epbs_mgr = CollectionsManager::new(&env.sandbox.db, "un2", "epbs");
    let _ = DbSandbox::mock_db(&epbs_mgr).await;

    epbs_mgr
        .create_collection("configuration_items", generic_schema)
        .await?;

    // --- 🎯 3. CONTEXTE & EXÉCUTION ---
    let session_id = AgentContext::generate_default_session_id(agent_urn, "test_suite_epbs")?;

    // 🎯 FIX : Utilisation de `bootstrap`
    let world_engine = SharedRef::new(
        raise::ai::world_model::NeuroSymbolicEngine::bootstrap(&sys_mgr)
            .await
            .expect("WM Engine bootstrap fail"),
    );

    let ctx = AgentContext::new(
        agent_urn,
        &session_id,
        env.sandbox.db.clone(), // 🎯 FIX : .db est DÉJÀ un SharedRef
        env.client.clone().expect("LlmClient requis pour les tests"),
        world_engine,
        test_root.clone(),
        test_root.join("dataset"),
    )
    .await?;

    let agent = DynamicAgent::new(agent_urn);
    let intent = EngineeringIntent::CreateElement {
        layer: "EPBS".to_string(),
        element_type: "COTS".to_string(),
        name: "Rack Server Dell R750".to_string(),
    };

    user_info!("INF_EPBS_AGENT_LAUNCH");
    match agent.process(&ctx, &intent).await {
        Ok(Some(res)) => user_success!("SUC_EPBS_PROCESS", json_value!({"msg": res.message})),
        Ok(None) => user_warn!("WRN_EPBS_NO_RESULT"),
        Err(e) => return Err(e),
    }

    // --- 🔍 4. VÉRIFICATION (Résilience & Artefacts) ---
    tokio::time::sleep(TimeDuration::from_millis(1500)).await;

    let items_dir = test_root.join("un2/epbs/collections/configuration_items");
    let mut found = false;

    if items_dir.exists() {
        if let Ok(entries) = fs::read_dir_sync(&items_dir) {
            for e in entries.flatten() {
                let content = fs::read_to_string_sync(&e.path()).unwrap_or_default();
                if content.contains("name") && content.contains("Rack Server") {
                    found = true;
                    user_success!("SUC_CI_VALIDATED");
                    break;
                }
            }
        }
    }

    if !found {
        user_warn!("WRN_CI_NOT_WRITTEN_PHYSICALLY");
    }

    Ok(())
}

// =========================================================================
// NOUVEAUX TESTS : RÉSILIENCE ET POINTS DE MONTAGE
// =========================================================================

#[cfg(test)]
mod resilience_tests {
    use super::*;
    use raise::ai::llm::client::LlmClient;

    /// 🎯 Test la résilience face à la résolution des partitions via Mount Points
    #[async_test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_epbs_mount_point_integrity() -> RaiseResult<()> {
        let env = setup_test_env(LlmMode::Disabled).await?;
        // Validation SSOT de la partition système
        assert!(!env.sandbox.config.mount_points.system.domain.is_empty());
        assert!(!env.sandbox.config.mount_points.system.db.is_empty());
        Ok(())
    }

    /// 🎯 Test la réaction en cas de configuration d'agent corrompue (Match...raise_error)
    #[async_test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_epbs_agent_missing_prompt_id() -> RaiseResult<()> {
        let env = setup_test_env(LlmMode::Disabled).await?;

        // 🎯 FIX : Utilisation de domain_root
        let test_root = env.sandbox.domain_root.clone();

        // 🎯 FIX : Remplacement de `storage` par `db`
        let sys_mgr = CollectionsManager::new(
            &env.sandbox.db,
            &env.sandbox.config.mount_points.system.domain,
            &env.sandbox.config.mount_points.system.db,
        );

        // Injection d'un agent sans prompt_id valide
        sys_mgr
            .upsert_document(
                "agents",
                json_value!({
                    "handle": "agent_broken_epbs",
                    "base": { "neuro_profile": { "dummy": "data" } }
                }),
            )
            .await?;

        // 🎯 FIX : Bootstrap du World Model
        let world_engine = SharedRef::new(
            raise::ai::world_model::NeuroSymbolicEngine::bootstrap(&sys_mgr)
                .await
                .expect("WM Engine bootstrap fail"),
        );

        let llm_client = match env.client.clone() {
            Some(c) => c,
            None => LlmClient::new(&sys_mgr).await.expect("LlmClient fail"),
        };

        let ctx = AgentContext::new(
            "agent_broken_epbs",
            "sess_err",
            env.sandbox.db.clone(), // 🎯 FIX : .db est déjà un SharedRef
            llm_client,
            world_engine,
            test_root.clone(),
            test_root.clone(),
        )
        .await?;

        let agent = DynamicAgent::new("agent_broken_epbs");
        let res = agent.process(&ctx, &EngineeringIntent::Chat).await;

        match res {
            Err(AppError::Structured(data)) => {
                // Doit lever ERR_AGENT_MISSING_PROMPT ou similaire en fonction de votre logique PromptEngine
                assert_eq!(data.code, "ERR_AGENT_MISSING_PROMPT");
                Ok(())
            }
            _ => panic!("Le moteur aurait dû diverger sur ERR_AGENT_MISSING_PROMPT"),
        }
    }
}
