// FICHIER : src-tauri/src/plugins/tests.rs
use super::manager::PluginManager;
use crate::json_db::storage::{JsonDbConfig, StorageEngine};
use serde_json::json;
use std::fs;
use tempfile::tempdir;

fn create_test_env() -> (PluginManager, StorageEngine, tempfile::TempDir) {
    let dir = tempdir().unwrap();
    let config = JsonDbConfig::new(dir.path().to_path_buf());
    let storage = StorageEngine::new(config);
    let manager = PluginManager::new(&storage);
    (manager, storage, dir)
}

// Fonction utilitaire pour générer le binaire WASM dynamiquement
// Cela évite les erreurs de comptage manuel d'octets
fn generate_spy_plugin_wasm() -> Vec<u8> {
    let mut wasm = Vec::new();

    // 1. HEADER (8 bytes)
    wasm.extend_from_slice(&[0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00]);

    // 2. SECTION 1 (Types): 2 types
    // Size: 11 bytes
    wasm.extend_from_slice(&[
        0x01, 0x0b, 0x02, 0x60, 0x02, 0x7f, 0x7f, 0x01, 0x7f, // (i32, i32) -> i32
        0x60, 0x00, 0x01, 0x7f, // () -> i32
    ]);

    // 3. SECTION 2 (Import): "env.host_db_read"
    // Size: 20 bytes
    wasm.extend_from_slice(&[
        0x02, 0x14, 0x01, 0x03, 0x65, 0x6e, 0x76, // "env"
        0x0c, 0x68, 0x6f, 0x73, 0x74, 0x5f, 0x64, 0x62, 0x5f, 0x72, 0x65, 0x61,
        0x64, // "host_db_read"
        0x00, 0x00, // kind func, index 0
    ]);

    // 4. SECTION 3 (Function): 1 function (Type 1)
    // Size: 2 bytes
    wasm.extend_from_slice(&[0x03, 0x02, 0x01, 0x01]);

    // 5. SECTION 5 (Memory): 1 page
    // Size: 3 bytes
    wasm.extend_from_slice(&[0x05, 0x03, 0x01, 0x00, 0x01]);

    // 6. SECTION 7 (Export): "run" -> func 1, "memory" -> mem 0
    // Size: 16 bytes
    wasm.extend_from_slice(&[
        0x07, 0x10, 0x02, 0x06, 0x6d, 0x65, 0x6d, 0x6f, 0x72, 0x79, 0x02,
        0x00, // export "memory"
        0x03, 0x72, 0x75, 0x6e, 0x00, 0x01, // export "run"
    ]);

    // 7. SECTION 10 (Code): Body
    // Size: 10 bytes
    wasm.extend_from_slice(&[
        0x0a, 0x0a, 0x01, 0x08, 0x00, // func body size 8
        0x41, 0x00, // i32.const 0 (ptr)
        0x41, 0x28, // i32.const 40 (len) -> Longueur exacte de la string JSON ci-dessous
        0x10, 0x00, // call func 0 (host_db_read)
        0x0b, // end
    ]);

    // 8. SECTION 11 (Data): JSON String
    // JSON: {"collection":"secrets","id":"agent_007"} (40 caractères exacts)
    let json_data = b"{\"collection\":\"secrets\",\"id\":\"agent_007\"}";
    let data_len = json_data.len() as u8; // 40

    // Calcul de la taille de la section:
    // 1(count) + 1(mem_idx) + 3(offset) + 1(vec_len) + 40(data) = 46 bytes
    let section_size = 1 + 1 + 3 + 1 + data_len;

    wasm.push(0x0b); // ID Section 11
    wasm.push(section_size); // Taille Section
    wasm.push(0x01); // Count 1
    wasm.push(0x00); // Memory Index 0
    wasm.extend_from_slice(&[0x41, 0x00, 0x0b]); // Offset: i32.const 0
    wasm.push(data_len); // Taille vecteur données
    wasm.extend_from_slice(json_data); // Les données elles-mêmes

    wasm
}

#[test]
fn test_plugin_lifecycle_and_cognitive_bridge() {
    let (manager, storage, _tmp_dir) = create_test_env();
    let space = "test_space";
    let db = "test_db";

    // 1. SETUP DB
    let col_mgr =
        crate::json_db::collections::manager::CollectionsManager::new(&storage, space, db);
    col_mgr.create_collection("secrets", None).unwrap();

    let doc = json!({
        "id": "agent_007",
        "name": "James Bond",
        "status": "Active"
    });
    col_mgr.insert_raw("secrets", &doc).expect("Insert failed");

    // 2. CREATE WASM (Génération dynamique fiable)
    let wasm_bytes = generate_spy_plugin_wasm();
    let wasm_path = _tmp_dir.path().join("spy_plugin.wasm");
    fs::write(&wasm_path, wasm_bytes).expect("Impossible d'écrire le fichier WASM");

    // 3. LOAD
    manager
        .load_plugin("spy_v1", wasm_path.to_str().unwrap(), space, db)
        .expect("Le chargement du plugin a échoué");

    // 4. RUN
    let result = manager.run_plugin("spy_v1");

    assert!(result.is_ok(), "L'exécution du plugin a planté");
    assert_eq!(result.unwrap(), 1, "Le plugin n'a pas réussi à lire la DB");
}

#[test]
fn test_plugin_not_found() {
    let (manager, _, _dir) = create_test_env();
    let res = manager.run_plugin("phantom_plugin");
    assert!(res.is_err());
}

#[test]
fn test_invalid_wasm_file() {
    let (manager, _, dir) = create_test_env();
    let bad_path = dir.path().join("bad.wasm");
    fs::write(&bad_path, b"not a wasm file").unwrap();

    let res = manager.load_plugin("bad", bad_path.to_str().unwrap(), "s", "d");
    assert!(res.is_err());
}
