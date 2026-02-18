// FICHIER : src-tauri/tests/json_db_suite/workunits_x_compute.rs

use crate::common::setup_test_env; // Notre socle SSOT unifié
use raise::json_db::collections::manager::{self, CollectionsManager};
use raise::json_db::schema::{SchemaRegistry, SchemaValidator};
use raise::utils::prelude::*; // Apporte json!, Value, Uuid, etc.

#[tokio::test]
async fn workunit_compute_then_validate_minimal() {
    // 1. Initialisation de l'environnement (Sandboxing total)
    let env = setup_test_env().await;

    // 2. Chargement du registre des schémas depuis la DB isolée
    let reg = SchemaRegistry::from_db(&env.storage.config, &env.space, &env.db)
        .await
        .expect("❌ Impossible de charger le registre des schémas");

    // Construction de l'URI SSOT
    let root_uri = format!(
        "db://{}/{}/schemas/v1/workunits/workunit.schema.json",
        env.space, env.db
    );

    if reg.get_by_uri(&root_uri).is_none() {
        panic!(
            "❌ Schéma workunit introuvable dans le registre : {}",
            root_uri
        );
    }

    let validator = SchemaValidator::compile_with_registry(&root_uri, &reg)
        .expect("❌ Échec de la compilation du validateur workunit");

    // 3. Donnée conforme au workunit.schema.json
    let doc = json!({
        "id": Uuid::new_v4().to_string(),
        "code": "WU-DEVOPS-01",
        "name": { "fr": "DevOps pipeline" },
        "status": "draft",
        "version": "1.0.0",
        "createdAt": chrono::Utc::now().to_rfc3339(),
        "finance": {
            "version": "1.0.0",
            "billing_model": "time_material",
            "revenue_scenarios": {},
            "gross_margin": {},
            "summary": {},
            "synthese_build": {}
        }
    });

    // 4. Validation structurelle simple
    validator
        .validate(&doc)
        .expect("❌ La validation simple du workunit a échoué");
    println!("✅ Workunit minimal validé.");
}

#[tokio::test]
async fn finance_compute_minimal() {
    // 1. Initialisation de l'environnement
    let env = setup_test_env().await;
    let mgr = CollectionsManager::new(&env.storage, &env.space, &env.db);

    let reg = SchemaRegistry::from_db(&env.storage.config, &env.space, &env.db)
        .await
        .expect("❌ Échec init registre");

    let root_uri = format!(
        "db://{}/{}/schemas/v1/workunits/finance.schema.json",
        env.space, env.db
    );

    let validator = SchemaValidator::compile_with_registry(&root_uri, &reg)
        .expect("❌ Échec compilation validateur finance");

    // 2. CAS DE TEST : Données brutes avant calcul
    let mut finance_doc = json!({
        "version": "1.0.0",
        "billing_model": "fixed",
        "revenue_scenarios": {
            "low_eur": 1000,
            "mid_eur": 2000,
            "high_eur": 3000
        },
        "gross_margin": {
            "low_pct": 0.20, // 20%
            "mid_pct": 0.50, // 50%
            "high_pct": 0.80
        },
        "summary": {}, // Destiné à recevoir les x_rules
        "synthese_build": {}
    });

    // 3. EXÉCUTION DU MOTEUR DE RÈGLES (Business Logic)
    // On applique les règles dynamiques définies dans le schéma
    manager::apply_business_rules(
        &mgr,
        "finance_test_collection",
        &mut finance_doc,
        None,
        &reg,
        &root_uri,
    )
    .await
    .expect("❌ Échec de l'application des règles métier (GenRules)");

    // 4. VALIDATION DU RÉSULTAT
    validator
        .validate(&finance_doc)
        .expect("❌ Le document calculé ne respecte plus son schéma");

    // 5. ASSERTIONS (Vérification des calculs x_rules)

    // Marge Low : 1000 * 0.20 = 200
    assert_eq!(
        finance_doc
            .pointer("/summary/net_margin_low")
            .and_then(|v| v.as_f64()),
        Some(200.0),
        "❌ Calcul de marge Low incorrect"
    );

    // Marge Mid : 2000 * 0.50 = 1000
    assert_eq!(
        finance_doc
            .pointer("/summary/net_margin_mid")
            .and_then(|v| v.as_f64()),
        Some(1000.0),
        "❌ Calcul de marge Mid incorrect"
    );

    // Profitabilité : 1000 > 0 -> true
    assert_eq!(
        finance_doc
            .pointer("/summary/mid_is_profitable")
            .and_then(|v| v.as_bool()),
        Some(true),
        "❌ Évaluation de profitabilité incorrecte"
    );

    // Référence générée : "FIN-2025-OK"
    assert_eq!(
        finance_doc
            .pointer("/summary/generated_ref")
            .and_then(|v| v.as_str()),
        Some("FIN-2025-OK"),
        "❌ Référence calculée incorrecte"
    );

    println!("✅ FINANCE RULES ENGINE SUCCESS");
}
