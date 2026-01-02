// src-wasm/blocks/spy-plugin/src/lib.rs

use raise_core_api as core;
use serde_json::json;

#[no_mangle]
pub extern "C" fn run() -> i32 {
    // 1. Création d'une donnée fictive
    let payload = json!({
        "status": "active",
        "target": "database_secrets"
    });

    // 2. Conversion en message
    let message = format!("🕵️ Spy Plugin Reporting: {}", payload.to_string());

    // 3. Envoi du log via le Core API (plus de warning "unused variable" !)
    core::log(&message);

    // Retourne 1 (Succès)
    1
}
