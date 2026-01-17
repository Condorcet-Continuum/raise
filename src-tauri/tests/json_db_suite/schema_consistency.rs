// FICHIER : src-tauri/tests/json_db_suite/schema_consistency.rs

use crate::{init_test_env, TEST_DB, TEST_SPACE};
use raise::json_db::jsonld::{JsonLdProcessor, VocabularyRegistry};
use raise::json_db::schema::{SchemaRegistry, SchemaValidator};
use serde_json::{json, Value};
use std::fs;
use walkdir::WalkDir;

#[tokio::test]
async fn test_structural_integrity_json_schema() {
    let env = init_test_env().await;
    let cfg = &env.cfg;

    let schemas_root = cfg.db_schemas_root(TEST_SPACE, TEST_DB).join("v1");

    // Le chargement du registre à partir des fichiers est synchrone
    let registry = SchemaRegistry::from_db(cfg, TEST_SPACE, TEST_DB)
        .expect("Impossible de charger le registre des schémas");

    let mut error_count = 0;
    let mut checked_count = 0;

    println!(
        "\n🔍 [STRUCTURAL] Vérification des schémas dans : {:?}",
        schemas_root
    );

    for entry in WalkDir::new(&schemas_root)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if path.extension().map_or(false, |e| e == "json") {
            let rel_path = path.strip_prefix(&schemas_root).unwrap();
            let rel_str = rel_path.to_string_lossy().replace("\\", "/");

            // CORRECTION : Construction manuelle de l'URI au lieu de registry.uri()
            let uri = format!("db://{}/{}/schemas/v1/{}", TEST_SPACE, TEST_DB, rel_str);

            match SchemaValidator::compile_with_registry(&uri, &registry) {
                Ok(_) => {}
                Err(e) => {
                    println!("❌ ERREUR sur '{}': {}", rel_str, e);
                    error_count += 1;
                }
            }
            checked_count += 1;
        }
    }

    println!("✅ {} schémas vérifiés.", checked_count);
    if error_count > 0 {
        panic!(
            "🚨 {} erreurs de compilation de schéma détectées.",
            error_count
        );
    }
}

#[tokio::test]
async fn test_semantic_consistency_json_ld() {
    let _env = init_test_env().await;

    let processor = JsonLdProcessor::new();
    let vocab_registry = VocabularyRegistry::new();

    let critical_mappings = vec![
        ("actors/actor.schema.json", "oa:OperationalActor"),
        ("arcadia/oa/actor.schema.json", "oa:OperationalActor"),
        (
            "arcadia/sa/system-function.schema.json",
            "sa:SystemFunction",
        ),
        (
            "arcadia/la/logical-component.schema.json",
            "la:LogicalComponent",
        ),
    ];

    let mut warnings = Vec::new();

    println!("\n🧠 [SEMANTIC] Vérification de la cohérence JSON-LD...");

    for (schema_rel, short_type) in critical_mappings {
        let doc = json!({
            "@context": {
                "oa": "https://raise.io/ontology/arcadia/oa#",
                "sa": "https://raise.io/ontology/arcadia/sa#",
                "la": "https://raise.io/ontology/arcadia/la#",
                "pa": "https://raise.io/ontology/arcadia/pa#"
            },
            "@type": short_type,
            "name": "Test Semantic"
        });

        // L'expansion JSON-LD et l'accès au vocabulaire sont synchrones
        let expanded = processor.expand(&doc);
        let type_uri = processor.get_type(&expanded);

        match type_uri {
            Some(uri) => {
                if !vocab_registry.has_class(&uri) {
                    warnings.push(format!(
                        "⚠️  Désynchronisation : Le type '{}' (Schéma {}) s'étend en '{}' qui est INCONNU du code Rust.", 
                        short_type, schema_rel, uri
                    ));
                }
            }
            None => {
                warnings.push(format!(
                    "❌ Expansion échouée pour le type '{}' dans {}",
                    short_type, schema_rel
                ));
            }
        }
    }

    if !warnings.is_empty() {
        for w in warnings {
            println!("{}", w);
        }
        panic!("🚨 Incohérences sémantiques détectées (voir logs ci-dessus).");
    }
}

#[tokio::test]
async fn test_detect_actor_duality() {
    let env = init_test_env().await;
    let cfg = &env.cfg;
    let schemas_root = cfg.db_schemas_root(TEST_SPACE, TEST_DB).join("v1");

    let generic_path = schemas_root.join("actors/actor.schema.json");
    let arcadia_path = schemas_root.join("arcadia/oa/actor.schema.json");

    if generic_path.exists() && arcadia_path.exists() {
        println!("\n⚠️  [AUDIT] Vérification de la distinction Acteur Générique vs Arcadia");

        let gen_json: Value =
            serde_json::from_str(&fs::read_to_string(generic_path).unwrap()).unwrap();
        let arc_json: Value =
            serde_json::from_str(&fs::read_to_string(arcadia_path).unwrap()).unwrap();

        let gen_props = gen_json["properties"].as_object().unwrap();
        let arc_props = arc_json["properties"].as_object().unwrap();

        let distinct =
            gen_props.contains_key("email") && arc_props.contains_key("allocatedActivities");

        assert!(distinct, "RISQUE MAJEUR : Les schémas d'acteurs semblent avoir fusionné ou perdu leurs distinctions !");
        println!("✅ Distinction confirmée : 'email' vs 'allocatedActivities'.");
    } else {
        println!("ℹ️  Pas de dualité détectée (fichiers manquants ?).");
    }
}
