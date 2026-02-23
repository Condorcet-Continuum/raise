// FICHIER : src-tauri/tests/json_db_suite/schema_consistency.rs

use crate::common::{setup_test_env, LlmMode};
use raise::json_db::jsonld::{JsonLdProcessor, VocabularyRegistry};
use raise::json_db::schema::{SchemaRegistry, SchemaValidator};
use raise::utils::io;
use raise::utils::prelude::*;
use walkdir::WalkDir; // Pour explorer les schémas récursivement

#[tokio::test]
async fn test_structural_integrity_json_schema() {
    // 1. Initialisation de l'environnement isolé (copie les schémas auto)
    let env = setup_test_env(LlmMode::Disabled).await;
    let cfg = &env.storage.config;

    let schemas_root = cfg.db_schemas_root(&env.space, &env.db).join("v1");

    // 2. Chargement du registre à partir des schémas copiés dans le sandbox
    let registry = SchemaRegistry::from_db(cfg, &env.space, &env.db)
        .await
        .expect("❌ Impossible de charger le registre des schémas");

    let mut error_count = 0;
    let mut checked_count = 0;

    println!(
        "\n🔍 [STRUCTURAL] Vérification des schémas dans : {:?}",
        schemas_root
    );

    // 3. Parcours récursif de tous les fichiers .json
    for entry in WalkDir::new(&schemas_root)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if path.extension().is_some_and(|e| e == "json") {
            let rel_path = path.strip_prefix(&schemas_root).unwrap();
            let rel_str = rel_path.to_string_lossy().replace("\\", "/");

            // Construction de l'URI interne db://
            let uri = format!("db://{}/{}/schemas/v1/{}", env.space, env.db, rel_str);

            // Tentative de compilation (vérifie les $ref et la syntaxe)
            match SchemaValidator::compile_with_registry(&uri, &registry) {
                Ok(_) => {}
                Err(e) => {
                    println!("❌ ERREUR de compilation sur '{}': {}", rel_str, e);
                    error_count += 1;
                }
            }
            checked_count += 1;
        }
    }

    println!("✅ {} schémas vérifiés.", checked_count);
    if error_count > 0 {
        panic!("🚨 {} erreurs de compilation de schéma détectées. Vérifiez la syntaxe et les dépendances ($ref).", error_count);
    }
}

#[tokio::test]
async fn test_semantic_consistency_json_ld() {
    // On initialise juste pour le logging et les utilitaires
    let _env = setup_test_env(LlmMode::Disabled).await;

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

        // Expansion JSON-LD
        let expanded = processor.expand(&doc);
        let type_uri = processor.get_type(&expanded);

        match type_uri {
            Some(uri) => {
                // Vérifie si l'URI expansée est connue de l'ontologie Rust
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
        panic!("🚨 Incohérences sémantiques détectées entre les schémas JSON et l'ontologie Rust.");
    }
}

#[tokio::test]
async fn test_detect_actor_duality() {
    let env = setup_test_env(LlmMode::Disabled).await;
    let cfg = &env.storage.config;
    let schemas_root = cfg.db_schemas_root(&env.space, &env.db).join("v1");

    let generic_path = schemas_root.join("actors/actor.schema.json");
    let arcadia_path = schemas_root.join("arcadia/oa/actor.schema.json");

    if io::exists(&generic_path).await && io::exists(&arcadia_path).await {
        println!("\n⚠️  [AUDIT] Vérification de la distinction Acteur Générique vs Arcadia");

        let gen_json: Value = io::read_json(&generic_path)
            .await
            .expect("❌ JSON générique illisible");
        let arc_json: Value = io::read_json(&arcadia_path)
            .await
            .expect("❌ JSON arcadia illisible");

        let gen_props = gen_json["properties"]
            .as_object()
            .expect("❌ Manque 'properties' dans acteur générique");
        let arc_props = arc_json["properties"]
            .as_object()
            .expect("❌ Manque 'properties' dans acteur arcadia");

        // Vérification de la distinction métier stricte
        let distinct =
            gen_props.contains_key("email") && arc_props.contains_key("allocatedActivities");

        assert!(distinct, "🚨 RISQUE MAJEUR : Les schémas d'acteurs ont perdu leurs distinctions (email vs allocatedActivities) !");
        println!("✅ Distinction métier confirmée.");
    } else {
        println!("ℹ️  Audit de dualité ignoré (fichiers non présents dans le dataset de test).");
    }
}
