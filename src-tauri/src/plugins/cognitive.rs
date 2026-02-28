// FICHIER : src-tauri/src/plugins/cognitive.rs

use crate::utils::prelude::*;

use super::runtime::PluginContext;
use crate::json_db::collections::manager::CollectionsManager;
use crate::model_engine::loader::ModelLoader;
use crate::rules_engine::ast::Rule;
use crate::rules_engine::store::RuleStore;

use futures::executor::block_on;
use serde_json::{json, Value};
use wasmtime::{Caller, Extern, Linker};

/// Enregistre les fonctions du Pont Cognitif dans le linker WASM.
pub fn register_host_functions(linker: &mut Linker<PluginContext>) -> RaiseResult<()> {
    // ========================================================================
    // 1. SYSTÈME & LOGS
    // ========================================================================
    if let Err(e) = linker.func_wrap(
        "env",
        "plugin_log",
        |mut caller: Caller<'_, PluginContext>, ptr: i32, len: i32| -> anyhow::Result<i32> {
            let mut execute_log = || -> RaiseResult<i32> {
                let mem = match get_memory(&mut caller) {
                    Some(m) => m,
                    None => raise_error!(
                        "ERR_WASM_MEMORY",
                        error = "Accès mémoire refusé",
                        context = json!({"ptr": ptr})
                    ),
                };
                let msg = match read_string_from_wasm(&mut caller, &mem, ptr, len) {
                    Ok(m) => m,
                    Err(e) => raise_error!(
                        "ERR_WASM_STRING",
                        error = format!("Chaîne corrompue: {}", e),
                        context = json!({"ptr": ptr})
                    ),
                };
                println!("🤖 [PLUGIN LOG]: {}", msg);
                Ok(0)
            };
            execute_log().map_err(|e| anyhow::anyhow!(e))
        },
    ) {
        raise_error!(
            "ERR_WASM_BINDING",
            error = e.to_string(),
            context = json!({"func": "plugin_log"})
        );
    }

    // ========================================================================
    // 2. GESTION DE LA MÉMOIRE & COMMUNICATION (Output)
    // ========================================================================

    if let Err(e) = linker.func_wrap(
        "env",
        "host_fetch_result",
        |mut caller: Caller<'_, PluginContext>, ptr: i32, max_len: i32| -> anyhow::Result<i32> {
            let mut execute_fetch = || -> RaiseResult<i32> {
                let data = caller.data().output_buffer.clone();
                let data_len = data.len();
                if data_len == 0 {
                    return Ok(0);
                }
                let mem = match get_memory(&mut caller) {
                    Some(m) => m,
                    None => raise_error!(
                        "ERR_WASM_MEMORY",
                        error = "Accès mémoire refusé",
                        context = json!({"ptr": ptr})
                    ),
                };
                let write_len = std::cmp::min(data_len, max_len as usize);
                match mem.write(&mut caller, ptr as usize, &data[0..write_len]) {
                    Ok(_) => Ok(write_len as i32),
                    Err(write_err) => raise_error!(
                        "ERR_WASM_WRITE",
                        error = write_err.to_string(),
                        context = json!({"ptr": ptr, "len": write_len})
                    ),
                }
            };
            execute_fetch().map_err(|e| anyhow::anyhow!(e))
        },
    ) {
        raise_error!(
            "ERR_WASM_BINDING",
            error = e.to_string(),
            context = json!({"func": "host_fetch_result"})
        );
    }

    if let Err(e) = linker.func_wrap(
        "env",
        "host_signal_event",
        |mut caller: Caller<'_, PluginContext>, ptr: i32, len: i32| -> anyhow::Result<i32> {
            let mut execute_signal = || -> RaiseResult<i32> {
                let req = match read_json_request(&mut caller, ptr, len) {
                    Ok(v) => v,
                    Err(err) => raise_error!(
                        "ERR_WASM_SIGNAL",
                        error = err.to_string(),
                        context = json!({"ptr": ptr})
                    ),
                };
                caller.data_mut().signals.push(req);
                Ok(1)
            };
            execute_signal().map_err(|e| anyhow::anyhow!(e))
        },
    ) {
        raise_error!(
            "ERR_WASM_BINDING",
            error = e.to_string(),
            context = json!({"func": "host_signal_event"})
        );
    }

    // ========================================================================
    // 3. BASE DE DONNÉES (SÉCURISÉE PAR LE MANDAT)
    // ========================================================================

    if let Err(e) = linker.func_wrap(
        "env",
        "host_db_read",
        |mut caller: Caller<'_, PluginContext>, ptr: i32, len: i32| -> anyhow::Result<i32> {
            let mut execute_db_read = || -> RaiseResult<i32> {
                let req = match read_json_request(&mut caller, ptr, len) {
                    Ok(v) => v,
                    Err(err) => return Ok(error_to_buffer(&mut caller, &err.to_string())),
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
                    Ok(Some(doc)) => Ok(success_to_buffer(&mut caller, doc)),
                    Ok(None) => Ok(success_to_buffer(&mut caller, Value::Null)),
                    Err(err) => Ok(error_to_buffer(&mut caller, &err.to_string())),
                }
            };
            execute_db_read().map_err(|e| anyhow::anyhow!(e))
        },
    ) {
        raise_error!(
            "ERR_WASM_BINDING",
            error = e.to_string(),
            context = json!({"func": "host_db_read"})
        );
    }

    if let Err(e) = linker.func_wrap(
        "env",
        "host_db_write",
        |mut caller: Caller<'_, PluginContext>, ptr: i32, len: i32| -> anyhow::Result<i32> {
            let mut execute_db_write = || -> RaiseResult<i32> {
                let req = match read_json_request(&mut caller, ptr, len) {
                    Ok(v) => v,
                    Err(err) => return Ok(error_to_buffer(&mut caller, &err.to_string())),
                };

                {
                    let ctx = caller.data();
                    if let Some(mandate) = &ctx.mandate {
                        if mandate
                            .get("readonly")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false)
                        {
                            return Ok(error_to_buffer(&mut caller, "MANDATE_VIOLATION_READONLY"));
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
                    Ok(id) => Ok(success_to_buffer(&mut caller, json!({ "inserted_id": id }))),
                    Err(err) => Ok(error_to_buffer(&mut caller, &err.to_string())),
                }
            };
            execute_db_write().map_err(|e| anyhow::anyhow!(e))
        },
    ) {
        raise_error!(
            "ERR_WASM_BINDING",
            error = e.to_string(),
            context = json!({"func": "host_db_write"})
        );
    }

    // ========================================================================
    // 4. SERVICES ÉTENDUS (AI, MODEL, RULES)
    // ========================================================================

    if let Err(e) = linker.func_wrap(
        "env",
        "host_llm_inference",
        |mut caller: Caller<'_, PluginContext>, ptr: i32, len: i32| -> anyhow::Result<i32> {
            let mut execute_llm = || -> RaiseResult<i32> {
                let req = match read_json_request(&mut caller, ptr, len) {
                    Ok(v) => v,
                    Err(err) => {
                        return Ok(error_to_buffer(
                            &mut caller,
                            &format!("Input Error: {}", err),
                        ))
                    }
                };

                let prompt = req["prompt"].as_str().unwrap_or("").to_string();
                let ai_opt = caller.data().ai_orchestrator.clone();

                let response_result = match ai_opt {
                    Some(orch_arc) => {
                        let mut orch = orch_arc.lock().unwrap();
                        block_on(orch.ask(&prompt))
                    }
                    None => Err(crate::build_error!(
                        "ERR_COGNITIVE_PLUGIN_AUTH",
                        context = json!({
                            "action": "validate_session",
                            "hint": "L'orchestrateur IA est absent."
                        })
                    )),
                };

                match response_result {
                    Ok(response) => Ok(success_to_buffer(
                        &mut caller,
                        json!({ "response": response }),
                    )),
                    Err(err) => {
                        eprintln!("[Plugin Error] {}", err);
                        Ok(error_to_buffer(&mut caller, &format!("{}", err)))
                    }
                }
            };
            execute_llm().map_err(|e| anyhow::anyhow!(e))
        },
    ) {
        raise_error!(
            "ERR_WASM_BINDING",
            error = e.to_string(),
            context = json!({"func": "host_llm_inference"})
        );
    }

    if let Err(e) = linker.func_wrap(
        "env",
        "host_model_query",
        |mut caller: Caller<'_, PluginContext>, ptr: i32, len: i32| -> anyhow::Result<i32> {
            let mut execute_model_query = || -> RaiseResult<i32> {
                let req = match read_json_request(&mut caller, ptr, len) {
                    Ok(v) => v,
                    Err(err) => {
                        return Ok(error_to_buffer(
                            &mut caller,
                            &format!("Input Error: {}", err),
                        ))
                    }
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
                    Ok(el) => Ok(success_to_buffer(&mut caller, json!(el))),
                    Err(err) => Ok(error_to_buffer(&mut caller, &err.to_string())),
                }
            };
            execute_model_query().map_err(|e| anyhow::anyhow!(e))
        },
    ) {
        raise_error!(
            "ERR_WASM_BINDING",
            error = e.to_string(),
            context = json!({"func": "host_model_query"})
        );
    }

    if let Err(e) = linker.func_wrap(
        "env",
        "host_rule_validate",
        |mut caller: Caller<'_, PluginContext>, ptr: i32, len: i32| -> anyhow::Result<i32> {
            let mut execute_rule_validate = || -> RaiseResult<i32> {
                let req = match read_json_request(&mut caller, ptr, len) {
                    Ok(v) => v,
                    Err(err) => {
                        return Ok(error_to_buffer(
                            &mut caller,
                            &format!("Input Error: {}", err),
                        ))
                    }
                };
                let target_filter = req["target"].as_str().unwrap_or("").to_string();
                let (storage, space, db) = {
                    let ctx = caller.data();
                    (ctx.storage.clone(), ctx.space.clone(), ctx.db.clone())
                };
                let result: RaiseResult<Vec<Rule>> = block_on(async move {
                    let mgr = CollectionsManager::new(&storage, &space, &db);
                    let mut store = RuleStore::new(&mgr);

                    // PLUS DE `?` ICI NON PLUS !
                    match store.sync_from_db().await {
                        Ok(_) => {}
                        Err(sync_err) => raise_error!(
                            "ERR_RULE_SYNC_FAILED",
                            error = sync_err.to_string(),
                            context = json!({"target": target_filter})
                        ),
                    };

                    let rules = if target_filter.is_empty() {
                        store.get_all_rules()
                    } else {
                        store.get_rules_for_target(&target_filter)
                    };
                    Ok(rules)
                });
                match result {
                    Ok(rules) => Ok(success_to_buffer(&mut caller, json!(rules))),
                    Err(err) => Ok(error_to_buffer(&mut caller, &err.to_string())),
                }
            };
            execute_rule_validate().map_err(|e| anyhow::anyhow!(e))
        },
    ) {
        raise_error!(
            "ERR_WASM_BINDING",
            error = e.to_string(),
            context = json!({"func": "host_rule_validate"})
        );
    }

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
) -> RaiseResult<String> {
    let data = match memory.data(&caller).get(ptr as usize..(ptr + len) as usize) {
        Some(bytes) => bytes,
        None => {
            raise_error!(
                "ERR_WASM_MEMORY_OUT_OF_BOUNDS",
                error = "Accès mémoire hors limites dans le module Wasm.",
                context = json!({
                    "ptr": ptr,
                    "len": len
                })
            )
        }
    };

    match String::from_utf8(data.to_vec()) {
        Ok(s) => Ok(s),
        Err(e) => {
            raise_error!(
                "ERR_WASM_UTF8_DECODE_FAILED",
                error = e.to_string(),
                context = json!({
                    "ptr": ptr,
                    "len": len,
                    "hint": "Données corrompues envoyées par le plugin."
                })
            )
        }
    }
}

fn read_json_request(
    caller: &mut Caller<'_, PluginContext>,
    ptr: i32,
    len: i32,
) -> RaiseResult<Value> {
    let mem = match get_memory(caller) {
        Some(m) => m,
        None => raise_error!(
            "ERR_WASM_NO_MEMORY",
            error = "Aucune mémoire exportée par le module Wasm.",
            context = json!({"ptr": ptr, "len": len})
        ),
    };

    // Plus de `?` ici !
    let json_str = match read_string_from_wasm(caller, &mem, ptr, len) {
        Ok(s) => s,
        Err(e) => raise_error!(
            "ERR_WASM_JSON_READ",
            error = e.to_string(),
            context = json!({"ptr": ptr})
        ),
    };

    // Plus de `?` ici non plus !
    match serde_json::from_str(&json_str) {
        Ok(v) => Ok(v),
        Err(e) => raise_error!(
            "ERR_JSON_PARSE_FAILED",
            error = e.to_string(),
            context = json!({"action": "read_json_request"})
        ),
    }
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
