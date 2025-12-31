use crate::{ensure_db_exists, init_test_env, TEST_DB, TEST_SPACE};
use raise::json_db::collections::manager; // On a besoin du manager pour le nouveau moteur
use raise::json_db::schema::{SchemaRegistry, SchemaValidator};
use serde_json::json;
use uuid::Uuid;

#[test]
fn workunit_compute_then_validate_minimal() {
    let test_env = init_test_env();
    let cfg = &test_env.cfg;
    let space = TEST_SPACE;
    let db = TEST_DB;

    ensure_db_exists(cfg, space, db);

    let reg = SchemaRegistry::from_db(cfg, space, db).expect("registry init failed");
    let root_uri = reg.uri("workunits/workunit.schema.json");

    if reg.get_by_uri(&root_uri).is_none() {
        panic!("Schéma workunit introuvable");
    }

    let validator =
        SchemaValidator::compile_with_registry(&root_uri, &reg).expect("compile workunit failed");

    // Donnée conforme au workunit.schema.json (qui inclut finance)
    let doc = json!({
        "id": Uuid::new_v4().to_string(),
        "code": "WU-DEVOPS-01",
        "name": { "fr": "DevOps pipeline" },
        "status": "draft",
        "version": "1.0.0",
        "createdAt": "2024-01-01T00:00:00Z",
        "finance": {
            "version": "1.0.0",
            "billing_model": "time_material",
            "revenue_scenarios": {},
            "gross_margin": {},
            "summary": {},
            "synthese_build": {}
        }
    });

    // NOTE : Pour l'instant, apply_business_rules ne descend pas récursivement dans les sous-objets (finance).
    // Ce test valide surtout la structure globale (validation de schéma).
    validator.validate(&doc).expect("validate workunit failed");

    // Le calcul de l'ID ou createdAt est géré par le validateur/compute legacy ou le code d'insertion.
    // Ici on vérifie juste que ça valide.
}

#[test]
fn finance_compute_minimal() {
    let env = init_test_env();
    let cfg = &env.cfg;
    let space = TEST_SPACE;
    let db = TEST_DB;

    ensure_db_exists(cfg, space, db);

    let reg = SchemaRegistry::from_db(cfg, space, db).expect("registry init failed");
    let root_uri = reg.uri("workunits/finance.schema.json"); // On teste le module finance directement

    if reg.get_by_uri(&root_uri).is_none() {
        panic!("Schéma finance introuvable");
    }

    let validator =
        SchemaValidator::compile_with_registry(&root_uri, &reg).expect("compile finance failed");

    // CAS DE TEST : Une finance avec des revenus et des marges
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
        "summary": {}, // Les résultats seront injectés ici
        "synthese_build": {}
    });

    // 1. APPEL DU NOUVEAU MOTEUR (GenRules via manager)
    // C'est ce qui remplace l'ancien "apply_x_compute" implicite.
    manager::apply_business_rules(
        cfg,
        space,
        db,
        "finance_test", // Nom collection fictif pour le test
        &mut finance_doc,
        None,
        &reg,
        &root_uri,
    )
    .expect("Echec du moteur de règles");

    // 2. VALIDATION (Vérifie que le résultat respecte le schéma)
    validator
        .validate(&finance_doc)
        .expect("Validation du résultat échouée");

    println!(
        "Doc calculé : {}",
        serde_json::to_string_pretty(&finance_doc).unwrap()
    );

    // 3. ASSERTIONS (Vérification des règles x_rules)

    // Règle : calc_margin_low = low_eur (1000) * low_pct (0.20) = 200
    let margin_low = finance_doc.pointer("/summary/net_margin_low");
    assert_eq!(
        margin_low.and_then(|v| v.as_f64()),
        Some(200.0),
        "Marge Low incorrecte"
    );

    // Règle : calc_margin_mid = mid_eur (2000) * mid_pct (0.50) = 1000
    let margin_mid = finance_doc.pointer("/summary/net_margin_mid");
    assert_eq!(
        margin_mid.and_then(|v| v.as_f64()),
        Some(1000.0),
        "Marge Mid incorrecte"
    );

    // Règle : check_profitability (1000 > 0 -> true)
    let is_profitable = finance_doc.pointer("/summary/mid_is_profitable");
    assert_eq!(
        is_profitable.and_then(|v| v.as_bool()),
        Some(true),
        "Profitabilité incorrecte"
    );

    // Règle : gen_finance_ref ("FIN-2025-" + "OK" car profitable)
    let generated_ref = finance_doc.pointer("/summary/generated_ref");
    assert_eq!(
        generated_ref.and_then(|v| v.as_str()),
        Some("FIN-2025-OK"),
        "Référence générée incorrecte"
    );
}
