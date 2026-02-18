// FICHIER : src-tauri/src/plugins/cognitive.rs

use crate::utils::prelude::*;

use super::runtime::PluginContext;
use crate::json_db::collections::manager::CollectionsManager;
use crate::model_engine::loader::ModelLoader;
use crate::rules_engine::ast::Rule;
use crate::rules_engine::store::RuleStore;

use futures::executor::block_on;

use wasmtime::{Caller, Extern, Linker};

/// Enregistre les fonctions du Pont Cognitif dans le linker WASM.
pub fn register_host_functions(linker: &mut Linker<PluginContext>) -> Result<()> {
    // ========================================================================
    // 1. SYSTÈME & LOGS
    // ========================================================================
    linker.func_wrap(
        "env",
        "plugin_log",
        |mut caller: Caller<'_, PluginContext>, ptr: i32, len: i32| -> i32 {
            let mem = match get_memory(&mut caller) {
                Some(m) => m,
                None => return -1,
            };
            if let Ok(msg) = read_string_from_wasm(&mut caller, &mem, ptr, len) {
                println!("🤖 [PLUGIN LOG]: {}", msg);
            }
            0
        },
    )?;

    // ========================================================================
    // 2. GESTION DE LA MÉMOIRE & COMMUNICATION (Output)
    // ========================================================================

    linker.func_wrap(
        "env",
        "host_fetch_result",
        |mut caller: Caller<'_, PluginContext>, ptr: i32, max_len: i32| -> i32 {
            let data = caller.data().output_buffer.clone();
            let data_len = data.len();
            if data_len == 0 {
                return 0;
            }
            let mem = match get_memory(&mut caller) {
                Some(m) => m,
                None => return -1,
            };
            let write_len = std::cmp::min(data_len, max_len as usize);
            if let Err(e) = mem.write(&mut caller, ptr as usize, &data[0..write_len]) {
                eprintln!("🔥 [WASM Error] Write output failed: {}", e);
                return -1;
            }
            write_len as i32
        },
    )?;

    // host_signal_event(ptr, len)
    // Permet au plugin d'émettre un signal (événement JSON) vers le Workflow Engine.
    linker.func_wrap(
        "env",
        "host_signal_event",
        |mut caller: Caller<'_, PluginContext>, ptr: i32, len: i32| -> i32 {
            let req = match read_json_request(&mut caller, ptr, len) {
                Ok(v) => v,
                Err(_) => return -1,
            };
            // Stockage du signal dans le vecteur du contexte (Input pour le Workflow)
            caller.data_mut().signals.push(req);
            1
        },
    )?;

    // ========================================================================
    // 3. BASE DE DONNÉES (SÉCURISÉE PAR LE MANDAT)
    // ========================================================================

    linker.func_wrap(
        "env",
        "host_db_read",
        |mut caller: Caller<'_, PluginContext>, ptr: i32, len: i32| -> i32 {
            let req = match read_json_request(&mut caller, ptr, len) {
                Ok(v) => v,
                Err(_) => return error_to_buffer(&mut caller, "Invalid Input"),
            };
            let col = req["collection"].as_str().unwrap_or("").to_string();
            let id = req["id"].as_str().unwrap_or("").to_string();
            let (storage, space, db) = {
                let ctx = caller.data();
                (ctx.storage.clone(), ctx.space.clone(), ctx.db.clone())
            };
            let result = block_on(async move {
                let mgr = CollectionsManager::new(&storage, &space, &db);
                mgr.get_document(&col, &id).await
            });
            match result {
                Ok(Some(doc)) => success_to_buffer(&mut caller, doc),
                Ok(None) => success_to_buffer(&mut caller, Value::Null),
                Err(e) => error_to_buffer(&mut caller, &e.to_string()),
            }
        },
    )?;

    linker.func_wrap(
        "env",
        "host_db_write",
        |mut caller: Caller<'_, PluginContext>, ptr: i32, len: i32| -> i32 {
            let req = match read_json_request(&mut caller, ptr, len) {
                Ok(v) => v,
                Err(_) => return error_to_buffer(&mut caller, "Invalid Input"),
            };

            // --- VÉRIFICATION GOUVERNANCE ---
            // On vérifie si le mandat injecté interdit explicitement l'écriture.
            {
                let ctx = caller.data();
                if let Some(mandate) = &ctx.mandate {
                    if mandate
                        .get("readonly")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false)
                    {
                        return error_to_buffer(&mut caller, "MANDATE_VIOLATION_READONLY");
                    }
                }
            }

            let col = req["collection"].as_str().unwrap_or("").to_string();
            let data = req["data"].clone();
            let (storage, space, db) = {
                let ctx = caller.data();
                (ctx.storage.clone(), ctx.space.clone(), ctx.db.clone())
            };
            let result = block_on(async move {
                let mgr = CollectionsManager::new(&storage, &space, &db);
                mgr.insert_raw(&col, &data).await
            });
            match result {
                Ok(id) => success_to_buffer(&mut caller, json!({ "inserted_id": id })),
                Err(e) => error_to_buffer(&mut caller, &e.to_string()),
            }
        },
    )?;

    // ========================================================================
    // 4. SERVICES ÉTENDUS (AI, MODEL, RULES)
    // ========================================================================

    linker.func_wrap(
        "env",
        "host_llm_inference",
        |mut caller: Caller<'_, PluginContext>, ptr: i32, len: i32| -> i32 {
            let req = match read_json_request(&mut caller, ptr, len) {
                Ok(v) => v,
                Err(_) => return error_to_buffer(&mut caller, "Invalid Input"),
            };
            let prompt = req["prompt"].as_str().unwrap_or("").to_string();
            let ai_opt = caller.data().ai_orchestrator.clone();
            let response_result = if let Some(orch_arc) = ai_opt {
                let mut orch = orch_arc.lock().unwrap();
                block_on(orch.ask(&prompt))
            } else {
                Err(crate::utils::error::AppError::Validation(
                    "AI Orchestrator not available".into(),
                ))
            };
            match response_result {
                Ok(response) => success_to_buffer(&mut caller, json!({ "response": response })),
                Err(e) => error_to_buffer(&mut caller, &format!("AI Error: {}", e)),
            }
        },
    )?;

    linker.func_wrap(
        "env",
        "host_model_query",
        |mut caller: Caller<'_, PluginContext>, ptr: i32, len: i32| -> i32 {
            let req = match read_json_request(&mut caller, ptr, len) {
                Ok(v) => v,
                Err(_) => return error_to_buffer(&mut caller, "Invalid Input"),
            };
            let target_id = req["id"].as_str().unwrap_or("").to_string();
            let (storage, space, db) = {
                let ctx = caller.data();
                (ctx.storage.clone(), ctx.space.clone(), ctx.db.clone())
            };
            let result = block_on(async move {
                let mgr = CollectionsManager::new(&storage, &space, &db);
                let loader = ModelLoader::new_with_manager(mgr);
                loader.get_element(&target_id).await
            });
            match result {
                Ok(el) => success_to_buffer(&mut caller, json!(el)),
                Err(e) => error_to_buffer(&mut caller, &e.to_string()),
            }
        },
    )?;

    linker.func_wrap(
        "env",
        "host_rule_validate",
        |mut caller: Caller<'_, PluginContext>, ptr: i32, len: i32| -> i32 {
            let req = match read_json_request(&mut caller, ptr, len) {
                Ok(v) => v,
                Err(_) => return error_to_buffer(&mut caller, "Invalid Input"),
            };
            let target_filter = req["target"].as_str().unwrap_or("").to_string();
            let (storage, space, db) = {
                let ctx = caller.data();
                (ctx.storage.clone(), ctx.space.clone(), ctx.db.clone())
            };
            let result: Result<Vec<Rule>> = block_on(async move {
                let mgr = CollectionsManager::new(&storage, &space, &db);
                let mut store = RuleStore::new(&mgr);
                store.sync_from_db().await?;
                let rules = if target_filter.is_empty() {
                    store.get_all_rules()
                } else {
                    store.get_rules_for_target(&target_filter)
                };
                Ok(rules)
            });
            match result {
                Ok(rules) => success_to_buffer(&mut caller, json!(rules)),
                Err(e) => error_to_buffer(&mut caller, &e.to_string()),
            }
        },
    )?;

    Ok(())
}

// --- HELPERS ---

fn get_memory(caller: &mut Caller<'_, PluginContext>) -> Option<wasmtime::Memory> {
    match caller.get_export("memory") {
        Some(Extern::Memory(m)) => Some(m),
        _ => None,
    }
}

fn read_string_from_wasm(
    caller: &mut Caller<'_, PluginContext>,
    memory: &wasmtime::Memory,
    ptr: i32,
    len: i32,
) -> Result<String> {
    let data = memory
        .data(&caller)
        .get(ptr as usize..(ptr + len) as usize)
        .ok_or(anyhow::anyhow!("Out of bounds"))?;
    String::from_utf8(data.to_vec()).map_err(|e| AppError::from(format!("Erreur UTF-8: {}", e)))
}

fn read_json_request(caller: &mut Caller<'_, PluginContext>, ptr: i32, len: i32) -> Result<Value> {
    let mem = get_memory(caller).ok_or(anyhow::anyhow!("No memory exported"))?;
    let json_str = read_string_from_wasm(caller, &mem, ptr, len)?;
    Ok(serde_json::from_str(&json_str)?)
}

fn success_to_buffer(caller: &mut Caller<'_, PluginContext>, data: Value) -> i32 {
    let json_bytes = data.to_string().into_bytes();
    let len = json_bytes.len() as i32;
    caller.data_mut().output_buffer = json_bytes;
    len
}

fn error_to_buffer(caller: &mut Caller<'_, PluginContext>, msg: &str) -> i32 {
    let json_bytes = json!({ "error": msg }).to_string().into_bytes();
    let len = json_bytes.len() as i32;
    caller.data_mut().output_buffer = json_bytes;
    len
}

// --- TESTS UNITAIRES ---

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json_db::storage::{JsonDbConfig, StorageEngine};
    use tempfile::tempdir;
    use wasmtime::Engine;

    #[test]
    fn test_register_functions_integrity() {
        let engine = Engine::default();
        let mut linker = Linker::new(&engine);
        let temp_dir = tempdir().unwrap();
        let config = JsonDbConfig::new(temp_dir.path().to_path_buf());
        let storage = StorageEngine::new(config);

        let _context = PluginContext {
            storage,
            space: "test_space".to_string(),
            db: "test_db".to_string(),
            ai_orchestrator: None,
            mandate: None,
            signals: Vec::new(),
            output_buffer: Vec::new(),
        };

        let result = register_host_functions(&mut linker);
        assert!(result.is_ok());
    }
}
