use genaptitude::json_db::{
    collections,
    storage::{file_storage, JsonDbConfig},
};
use serde_json::json;
use std::path::Path;

#[test]
fn insert_actor_flow() {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let cfg = JsonDbConfig::from_env(repo_root).unwrap();
    let (space, db) = ("un2", "_system");
    let schema_rel = "actors/actor.schema.json";

    // Création DB/collection (idempotent)
    let _ = file_storage::create_db(&cfg, space, db);
    collections::create_collection(&cfg, space, db, "actors").unwrap();

    // Insert avec schéma (x_compute + validate)
    let doc = json!({
      "handle":"devops-engineer",
      "displayName":"Ingénieur DevOps",
      "label":{"fr":"Ingénieur DevOps","en":"DevOps Engineer"},
      "emoji":"🛠️","kind":"human","tags":["core"]
    });
    let stored = collections::insert_with_schema(&cfg, space, db, schema_rel, doc).unwrap();

    // Get par id
    let id = stored.get("id").and_then(|v| v.as_str()).unwrap();
    let loaded = collections::get(&cfg, space, db, "actors", id).unwrap();
    assert_eq!(loaded.get("id"), stored.get("id"));
}
