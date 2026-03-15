// FICHIER : src-tauri/tests/json_db_suite/schema_consistency.rs
use raise::utils::prelude::*;

use crate::common::{setup_test_env, LlmMode};
use raise::json_db::jsonld::JsonLdProcessor;
use raise::json_db::schema::{SchemaRegistry, SchemaValidator};

use walkdir::WalkDir; // Pour explorer les schémas récursivement

#[async_test]
async fn test_structural_integrity_json_schema() {
    // 1. Initialisation de l'environnement isolé (copie les schémas auto)
    let env = setup_test_env(LlmMode::Disabled).await;
    let cfg = &env.sandbox.storage.config;

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

#[async_test]
async fn test_semantic_consistency_json_ld() {
    let _env = setup_test_env(LlmMode::Disabled).await;

    // 1. PUISQU'IL N'Y A PAS DE FICHIERS D'ONTOLOGIE DANS LES TESTS :
    // On configure le processeur avec un contexte simulé pour l'expansion.
    let context_doc = json_value!({
        "@context": {
            "oa": "https://raise.io/ontology/arcadia/oa#",
            "sa": "https://raise.io/ontology/arcadia/sa#",
            "la": "https://raise.io/ontology/arcadia/la#",
            "pa": "https://raise.io/ontology/arcadia/pa#"
        }
    });

    let processor = JsonLdProcessor::new()
        .with_doc_context(&context_doc)
        .unwrap();

    // 2. On définit en dur les URIs complètes attendues (simulation du registre)
    let expected_uris = vec![
        "https://raise.io/ontology/arcadia/oa#OperationalActor",
        "https://raise.io/ontology/arcadia/sa#SystemFunction",
        "https://raise.io/ontology/arcadia/la#LogicalComponent",
    ];

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

    println!("\n🧠 [SEMANTIC] Vérification de la cohérence JSON-LD (Mode Simulé sans fichiers)...");

    for (schema_rel, short_type) in critical_mappings {
        let doc = json_value!({
            "@type": short_type,
            "name": "Test Semantic"
        });

        // 3. Expansion JSON-LD
        let expanded = processor.expand(&doc);
        let type_uri = processor.get_primary_type(&expanded);

        match type_uri {
            Some(uri) => {
                // 4. On vérifie simplement que l'URI expansée correspond à nos attentes
                if !expected_uris.contains(&uri.as_str()) {
                    warnings.push(format!(
                        "⚠️  Désynchronisation : Le type '{}' (Schéma {}) s'étend en '{}' qui n'est pas attendu.", 
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
        panic!("🚨 Incohérences sémantiques détectées lors de l'expansion.");
    }
}

#[async_test]
async fn test_detect_actor_duality() {
    let env = setup_test_env(LlmMode::Disabled).await;
    let cfg = &env.sandbox.storage.config;
    let schemas_root = cfg.db_schemas_root(&env.space, &env.db).join("v1");

    let generic_path = schemas_root.join("actors/actor.schema.json");
    let arcadia_path = schemas_root.join("arcadia/oa/actor.schema.json");

    if fs::exists_async(&generic_path).await && fs::exists_async(&arcadia_path).await {
        println!("\n⚠️  [AUDIT] Vérification de la distinction Acteur Générique vs Arcadia");

        let gen_json: JsonValue = fs::read_json_async(&generic_path)
            .await
            .expect("❌ JSON générique illisible");
        let arc_json: JsonValue = fs::read_json_async(&arcadia_path)
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
