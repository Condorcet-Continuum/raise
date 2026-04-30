// FICHIER : src-tauri/tests/json_db_suite/schema_minimal.rs

use crate::common::{setup_test_env, LlmMode};
use raise::json_db::schema::{SchemaRegistry, SchemaValidator};
use raise::utils::prelude::*; // Apporte json!, JsonValue, etc.

#[async_test]
async fn schema_instantiate_validate_minimal() -> RaiseResult<()> {
    // 1. Initialisation de l'environnement (Sandboxing total)
    let env = setup_test_env(LlmMode::Disabled).await?;
    let cfg = &env.sandbox.db.config;

    // 2. Chargement du registre des schémas depuis la DB isolée
    let reg = SchemaRegistry::from_db(cfg, &env.space, &env.db)
        .await
        .expect("❌ Impossible de charger le registre des schémas depuis la DB");

    // Construction de l'URI du schéma (Format SSOT)
    let collection_name = "mock_actors";
    let root_uri = "db://_system/_system/schemas/v1/mock/actors.schema.json".to_string();

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
    let mut doc = json_value!({
      "$schema": root_uri,
      "_id": UniqueId::new_v4().to_string(),
      "createdAt": UtcClock::now().to_rfc3339(),
      "updatedAt": UtcClock::now().to_rfc3339(),

      "handle": "devops-engineer",
      "displayName": "Ingénieur DevOps",
      "label": { "fr": "Ingénieur DevOps", "en": "DevOps Engineer" },
      "emoji": "🛠️",
      "kind": "human",
      "tags": ["core"]
    });

    // 4. Exécution du cycle complet : Calcul des x_props + Validation
    // 🎯 Instanciation du contexte pour le test (Zero Dette)
    let compute_ctx = raise::rules_engine::compute::ComputeContext {
        document: doc.clone(),
        collection_name: collection_name.to_string(),
        db_name: env.db.clone(),
        space_name: env.space.clone(),
    };

    // 4. Exécution du cycle complet : Calcul des x_props + Validation
    // 🎯 On passe le contexte ici !
    match validator
        .compute_then_validate(&mut doc, &compute_ctx)
        .await
    {
        Ok(_) => (),
        Err(e) => {
            raise_error!(
                "ERR_DB_COMPUTE_VALIDATE_FAIL",
                error = e,
                context = json_value!({
                    "collection": collection_name,
                    "root_uri": root_uri,
                    "action": "execute_schema_logic",
                    "hint": "Échec de l'hydratation dynamique. Vérifiez les opérateurs x_compute dans actors.schema.json."
                })
            );
        }
    }

    // 5. Vérifications finales de persistance des métadonnées
    assert!(
        doc.get("_id").is_some(),
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

    Ok(())
}
