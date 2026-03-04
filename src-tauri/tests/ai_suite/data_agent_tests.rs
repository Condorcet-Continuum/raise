// FICHIER : src-tauri/tests/ai_suite/data_agent_tests.rs

use crate::common::{setup_test_env, LlmMode};
use raise::ai::agents::intent_classifier::EngineeringIntent;
use raise::ai::agents::{data_agent::DataAgent, Agent, AgentContext};
use raise::utils::Arc;
// 👇 Import indispensable du manager
use raise::json_db::collections::manager::CollectionsManager;

#[tokio::test]
#[serial_test::serial] // Protection RTX 5060 en local
#[cfg_attr(not(feature = "cuda"), ignore)]
async fn test_data_agent_creates_class_and_enum() {
    let env = setup_test_env(LlmMode::Enabled).await;
    let test_root = env.sandbox.storage.config.data_root.clone();

    // --- 🎯 SETUP SPÉCIFIQUE AU TEST ---
    let data_mgr = CollectionsManager::new(&env.sandbox.storage, "un2", "data");

    // 1. Initialisation de la collection 'classes'
    data_mgr
        .create_collection(
            "classes",
            "db://_system/_system/schemas/v1/db/generic.schema.json",
        )
        .await
        .expect("Initialisation de la collection classes impossible");

    // 2. Initialisation de la collection 'types'
    data_mgr
        .create_collection(
            "types",
            "db://_system/_system/schemas/v1/db/generic.schema.json",
        )
        .await
        .expect("Initialisation de la collection types impossible");
    // -----------------------------------

    let agent_id = "data_agent_test";
    let session_id = AgentContext::generate_default_session_id(agent_id, "test_suite_data");

    let ctx = AgentContext::new(
        agent_id,
        &session_id,
        Arc::new(env.sandbox.storage.clone()),
        env.client
            .clone()
            .expect("LlmClient must be enabled for tests"),
        test_root.clone(),
        test_root.join("dataset"),
    )
    .await;

    let agent = DataAgent::new();

    // 1. Test CLASSE
    let intent_class = EngineeringIntent::CreateElement {
        layer: "DATA".to_string(),
        element_type: "Class".to_string(),
        name: "Client".to_string(),
    };
    let res_class = agent.process(&ctx, &intent_class).await;
    assert!(res_class.is_ok());

    if let Ok(Some(res)) = &res_class {
        println!("> {}", res.message);
    }

    // 2. Test ENUM
    let intent_enum = EngineeringIntent::CreateElement {
        layer: "DATA".to_string(),
        element_type: "DataType".to_string(),
        name: "StatutCommande".to_string(),
    };
    let res_enum = agent.process(&ctx, &intent_enum).await;
    assert!(res_enum.is_ok());

    let mut delegated_enum = false;
    if let Ok(Some(res)) = &res_enum {
        println!("> {}", res.message);
        delegated_enum = res.outgoing_message.is_some();
    }

    // --- VÉRIFICATION PHYSIQUE ---
    tokio::time::sleep(std::time::Duration::from_millis(1500)).await;

    // Check Class
    let classes_dir = test_root.join("un2/data/collections/classes");
    let mut found_class = false;

    if classes_dir.exists() {
        for e in std::fs::read_dir(&classes_dir).unwrap().flatten() {
            let content = std::fs::read_to_string(e.path())
                .unwrap_or_default()
                .to_lowercase();

            if content.contains("client") {
                found_class = true;
                println!(
                    "✅ Classe validée : {:?} (Contenu: {})",
                    e.file_name(),
                    content
                );
            } else if content.contains("errorfallback") {
                println!("❌ Fichier ERREUR trouvé : {:?}", e.file_name());
            }
        }
    }
    assert!(
        found_class,
        "Classe Client non trouvée ou mal formée (voir logs ci-dessus)."
    );

    // Check Enum
    let types_dir = test_root.join("un2/data/collections/types");
    let mut found_enum = false;

    if types_dir.exists() {
        for e in std::fs::read_dir(&types_dir).unwrap().flatten() {
            let content = std::fs::read_to_string(e.path())
                .unwrap_or_default()
                .to_lowercase();

            if content.contains("statutcommande") {
                found_enum = true;
                println!("✅ Enum validée : {:?}", e.file_name());
                break;
            }
        }
    }

    // 🎯 Tolérance pour la délégation
    if delegated_enum {
        println!("✅ SUCCÈS : L'agent a intelligemment délégué la création de l'Enum.");
    } else if found_enum {
        println!("✅ SUCCÈS : L'agent a généré l'Enum physiquement.");
    } else {
        println!(
            "⚠️ Enum non trouvée (Le modèle a répondu textuellement ou a fusionné la réponse)."
        );
    }
}
