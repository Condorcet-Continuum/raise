use genaptitude::json_db::schema::{SchemaRegistry, SchemaValidator};
use genaptitude::json_db::storage::{file_storage, JsonDbConfig};
use serde_json::json;
use std::path::Path;

#[test]
fn schema_instantiate_validate_minimal() {
    // 1) Localise la racine repo pour from_env(&Path)
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("parent of src-tauri");

    // 2) Config + DB
    let cfg = JsonDbConfig::from_env(repo_root).expect("cfg from env");
    let space = "un2";
    let db = "_system";
    let _ = file_storage::create_db(&cfg, space, db);

    // 3) Registre strict DB + compilateur
    let reg = SchemaRegistry::from_db(&cfg, space, db).expect("registry from DB");
    let root_uri = reg.uri("actors/actor.schema.json");
    let validator = SchemaValidator::compile_with_registry(&root_uri, &reg).expect("compile");

    // 4) Document minimal volontairement SANS id/createdAt/updatedAt
    let mut doc = json!({
      "handle": "devops-engineer",
      "displayName": "Ingénieur DevOps",
      "label": { "fr": "Ingénieur DevOps", "en": "DevOps Engineer" },
      "emoji": "🛠️",
      "kind": "human",
      "tags": ["core"]
    });

    // 5) Déclenche les x_compute (uuid_v4, now_ts_ms, etc.) PUIS valide
    validator
        .compute_then_validate(&mut doc)
        .expect("compute + validate ok");

    // 6) (facultatif) Vérifie que les champs calculés existent
    assert!(
        doc.get("_id").or_else(|| doc.get("id")).is_some(),
        "id/_id doit être calculé"
    );
    assert!(
        doc.get("createdAt").is_some(),
        "createdAt doit être calculé"
    );
    assert!(
        doc.get("updatedAt").is_some(),
        "updatedAt doit être calculé"
    );

    println!("doc après compute: {doc}");
}
