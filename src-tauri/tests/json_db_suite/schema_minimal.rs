// FICHIER : src-tauri/tests/json_db_suite/schema_minimal.rs

use crate::common::setup_test_env; // Notre socle SSOT
use raise::json_db::schema::{SchemaRegistry, SchemaValidator};
use raise::utils::prelude::*; // Apporte json!, Value, etc.

#[tokio::test]
async fn schema_instantiate_validate_minimal() {
    // 1. Initialisation de l'environnement (Sandboxing total)
    let env = setup_test_env().await;
    let cfg = &env.storage.config;

    // 2. Chargement du registre des schémas depuis la DB isolée
    let reg = SchemaRegistry::from_db(cfg, &env.space, &env.db)
        .await
        .expect("❌ Impossible de charger le registre des schémas depuis la DB");

    // Construction de l'URI du schéma (Format SSOT)
    let schema_rel_path = "actors/actor.schema.json";
    let root_uri = format!(
        "db://{}/{}/schemas/v1/{}",
        env.space, env.db, schema_rel_path
    );

    // Vérification de présence dans le registre
    if reg.get_by_uri(&root_uri).is_none() {
        panic!(
            "❌ Schéma introuvable dans le registre de test: {}",
            root_uri
        );
    }

    // Compilation du validateur avec le contexte du registre
    let validator = SchemaValidator::compile_with_registry(&root_uri, &reg)
        .expect("❌ Échec de la compilation du validateur de schéma");

    // 3. Construction d'un document minimal conforme
    // On simule un document qui possède les champs système requis par base.schema.json
    let mut doc = json!({
      "$schema": root_uri,
      "id": uuid::Uuid::new_v4().to_string(),
      "createdAt": chrono::Utc::now().to_rfc3339(),
      "updatedAt": chrono::Utc::now().to_rfc3339(),

      "handle": "devops-engineer",
      "displayName": "Ingénieur DevOps",
      "label": { "fr": "Ingénieur DevOps", "en": "DevOps Engineer" },
      "emoji": "🛠️",
      "kind": "human",
      "tags": ["core"]
    });

    // 4. Exécution du cycle complet : Calcul des x_props + Validation
    validator
        .compute_then_validate(&mut doc)
        .expect("❌ Le cycle compute_then_validate a échoué pour le document minimal");

    // 5. Vérifications finales de persistance des métadonnées
    assert!(
        doc.get("id").is_some(),
        "❌ L'ID a disparu après validation"
    );
    assert!(
        doc.get("createdAt").is_some(),
        "❌ createdAt a disparu après validation"
    );
    assert!(
        doc.get("updatedAt").is_some(),
        "❌ updatedAt a disparu après validation"
    );

    println!("✅ SCHEMA MINIMAL INSTANTIATION SUCCESS");
}
