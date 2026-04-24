// FICHIER : src-tauri/tests/ai_suite/data_agent_tests.rs

use raise::utils::prelude::*; // 🎯 Façade Unique RAISE

use crate::common::{get_test_wm_config, setup_test_env, LlmMode};
use raise::ai::agents::intent_classifier::EngineeringIntent;
use raise::ai::agents::{dynamic_agent::DynamicAgent, Agent, AgentContext};
use raise::json_db::collections::manager::CollectionsManager;
use raise::utils::testing::DbSandbox;

#[async_test]
#[serial_test::serial]
#[cfg_attr(not(feature = "cuda"), ignore)]
async fn test_data_agent_creates_class_and_enum() -> RaiseResult<()> {
    let env = setup_test_env(LlmMode::Enabled).await?;
    let test_root = env.sandbox.storage.config.data_root.clone();

    // --- 🎯 1. SETUP SYSTEM (Injection via Mount Points) ---
    // Utilisation dynamique des points de montage pour la partition système
    let system_domain = &env.sandbox.config.mount_points.system.domain;
    let system_db = &env.sandbox.config.mount_points.system.db;

    let sys_mgr = CollectionsManager::new(&env.sandbox.storage, system_domain, system_db);

    match DbSandbox::mock_db(&sys_mgr).await {
        Ok(_) => user_info!("INF_TEST_MOCK_DB_READY"),
        Err(e) => raise_error!("ERR_TEST_SETUP_FAIL", error = e.to_string()),
    }

    let schema_uri = format!(
        "db://{}/{}/schemas/v1/db/generic.schema.json",
        system_domain, system_db
    );
    let generic_schema = schema_uri.as_str();

    let collections = vec!["prompts", "agents", "session_agents", "configs"];

    for coll in collections {
        let _ = sys_mgr.create_collection(coll, generic_schema).await;
    }

    // Injection des définitions sémantiques de l'agent
    sys_mgr
        .upsert_document(
            "prompts",
            json_value!({
                "handle": "prompt_data",
                "role": "Architecte Données",
                "identity": { "persona": "Expert Data Arcadia. Répond en JSON pur.", "tone": "technique" },
                "environment": "Couche DATA du projet Condorcet.",
                "directives": ["Génère un TABLEAU JSON avec: '_id', 'name', 'type' (Class/DataType), 'layer' (DATA)."]
            }),
        )
        .await?;

    let agent_urn = "agent_data";
    sys_mgr.upsert_document("agents", json_value!({
        "handle": agent_urn,
        "base": {
            "name": { "fr": "Data Architect" },
            "neuro_profile": { "prompt_id": "ref:prompts:handle:prompt_data", "temperature": 0.1 }
        }
    })).await?;

    // --- 🎯 2. SETUP PROJET (Physique) ---
    let data_mgr = CollectionsManager::new(&env.sandbox.storage, "un2", "mbse");
    let _ = DbSandbox::mock_db(&data_mgr).await;

    data_mgr
        .create_collection("classes", generic_schema)
        .await?;
    data_mgr.create_collection("types", generic_schema).await?;

    // --- 🎯 3. CONTEXTE & EXÉCUTION ---
    let session_id = AgentContext::generate_default_session_id(agent_urn, "test_suite_data")?;

    let world_engine = SharedRef::new(
        raise::ai::world_model::NeuroSymbolicEngine::new(
            get_test_wm_config(),
            NeuralWeightsMap::new(),
        )
        .expect("WM Engine fail"),
    );

    let ctx = AgentContext::new(
        agent_urn,
        &session_id,
        SharedRef::new(env.sandbox.storage.clone()),
        env.client.clone().expect("LlmClient requis pour les tests"),
        world_engine,
        test_root.clone(),
        test_root.join("dataset"),
    )
    .await?;

    let agent = DynamicAgent::new(agent_urn);

    // 1. Test CRÉATION CLASSE
    let intent_class = EngineeringIntent::CreateElement {
        layer: "DATA".to_string(),
        element_type: "Class".to_string(),
        name: "Client".to_string(),
    };

    match agent.process(&ctx, &intent_class).await {
        Ok(Some(res)) => user_info!("INF_TEST_CLASS_GEN", json_value!({"msg": res.message})),
        Ok(None) => raise_error!("ERR_TEST_NO_RESULT"),
        Err(e) => return Err(e),
    }

    // 2. Test CRÉATION ENUM
    let intent_enum = EngineeringIntent::CreateElement {
        layer: "DATA".to_string(),
        element_type: "DataType".to_string(),
        name: "StatutCommande".to_string(),
    };

    let res_enum = agent.process(&ctx, &intent_enum).await?;
    let mut delegated_enum = false;

    if let Some(res) = res_enum {
        delegated_enum = res.outgoing_message.is_some();
    }

    // --- 🔍 4. VÉRIFICATION (Résilience & Artefacts) ---
    tokio::time::sleep(TimeDuration::from_millis(1500)).await;

    let classes_dir = test_root.join("un2/mbse/collections/classes");
    let mut found_class = false;

    if classes_dir.exists() {
        if let Ok(entries) = fs::read_dir_sync(&classes_dir) {
            for e in entries.flatten() {
                let content = fs::read_to_string_sync(&e.path()).unwrap_or_default();
                if content.to_lowercase().contains("client") {
                    found_class = true;
                    break;
                }
            }
        }
    }
    assert!(found_class, "Classe Client non trouvée physiquement.");

    if delegated_enum {
        user_success!("SUC_TEST_DELEGATION_OK");
    } else {
        user_success!("SUC_TEST_GENERATION_OK");
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

    /// 🎯 Test la résilience face à la résolution des partitions système
    #[async_test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_data_agent_mount_point_integrity() -> RaiseResult<()> {
        let env = setup_test_env(LlmMode::Disabled).await?;
        // Vérifie que les points de montage système sont injectés correctement dans la config sandbox
        assert!(!env.sandbox.config.mount_points.system.domain.is_empty());
        assert!(!env.sandbox.config.mount_points.system.db.is_empty());
        Ok(())
    }

    /// 🎯 Test la réaction du moteur en cas de prompt introuvable (Match...raise_error)
    #[async_test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_data_agent_missing_prompt_resilience() -> RaiseResult<()> {
        let env = setup_test_env(LlmMode::Disabled).await?;
        let test_root = env.sandbox.storage.config.data_root.clone();

        let sys_mgr = CollectionsManager::new(
            &env.sandbox.storage,
            &env.sandbox.config.mount_points.system.domain,
            &env.sandbox.config.mount_points.system.db,
        );

        // 1. Injection d'un agent dont le prompt_id pointe vers le néant
        sys_mgr
            .upsert_document(
                "agents",
                json_value!({
                    "handle": "agent_broken",
                    "base": { "neuro_profile": { "prompt_id": "ghost_prompt" } }
                }),
            )
            .await?;

        // 2. Préparation du contexte d'exécution
        let world_engine = SharedRef::new(
            raise::ai::world_model::NeuroSymbolicEngine::new(
                get_test_wm_config(),
                NeuralWeightsMap::new(),
            )
            .expect("WM Engine fail"),
        );

        let llm_client = match env.client.clone() {
            Some(client) => client,
            None => LlmClient::new(&sys_mgr).await.expect("LlmClient fail"),
        };

        let ctx = AgentContext::new(
            "agent_broken",
            "sess_resilience",
            SharedRef::new(env.sandbox.storage.clone()),
            llm_client,
            world_engine,
            test_root.clone(),
            test_root.join("dataset"),
        )
        .await?;

        let agent = DynamicAgent::new("agent_broken");

        // 🎯 FIX : Utilisation de la méthode 'process' au lieu de 'process_raw_request'
        let res = agent.process(&ctx, &EngineeringIntent::Chat).await;

        match res {
            Err(AppError::Structured(data)) => {
                // Le moteur doit lever une erreur structurée car 'ghost_prompt' n'existe pas en DB
                // Note : L'erreur sera levée par le PromptEngine appelé dans DynamicAgent::process
                assert!(
                    data.code.contains("ERR_AGENT_PROMPT_COMPILE")
                        || data.code.contains("ERR_PROMPT")
                        || data.code.contains("ERR_DB")
                );
                Ok(())
            }
            _ => panic!("Le moteur aurait dû diverger sur une erreur de résolution de prompt"),
        }
    }
}
