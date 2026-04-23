// FICHIER : src-tauri/src/plugins/cognitive.rs

use crate::utils::prelude::*; // 🎯 Façade Unique RAISE

use super::runtime::PluginContext;
use crate::json_db::collections::manager::CollectionsManager;
use crate::model_engine::loader::ModelLoader;
use crate::rules_engine::ast::Rule;
use crate::rules_engine::store::RuleStore;

use futures::executor::block_on;
use wasmtime::{Caller, Extern, Linker};

/// Enregistre les fonctions du Pont Cognitif dans le linker WASM.
pub fn register_host_functions(linker: &mut Linker<PluginContext>) -> RaiseResult<()> {
    // ========================================================================
    // 1. SYSTÈME & LOGS
    // ========================================================================
    if let Err(e) = linker.func_wrap(
        "env",
        "plugin_log",
        |mut caller: Caller<'_, PluginContext>, ptr: i32, len: i32| -> wasmtime::Result<i32> {
            let mut execute_log = || -> RaiseResult<i32> {
                let mem = match get_memory(&mut caller) {
                    Some(m) => m,
                    None => raise_error!("ERR_WASM_MEMORY", error = "Accès mémoire refusé"),
                };
                let msg = match read_string_from_wasm(&mut caller, &mem, ptr, len) {
                    Ok(m) => m,
                    Err(e) => raise_error!("ERR_WASM_STRING", error = e.to_string()),
                };
                user_info!("PLUGIN_LOG", json_value!({ "message": msg })); // 🎯 Traçabilité sémantique
                Ok(0)
            };

            match execute_log() {
                Ok(res) => Ok(res),
                Err(e) => Err(wasmtime::Error::msg(e.to_string())),
            }
        },
    ) {
        raise_error!(
            "ERR_WASM_BINDING",
            error = e.to_string(),
            context = json_value!({"func": "plugin_log"})
        );
    }

    // ========================================================================
    // 2. GESTION DE LA MÉMOIRE & COMMUNICATION (Output)
    // ========================================================================

    if let Err(e) = linker.func_wrap(
        "env",
        "host_fetch_result",
        |mut caller: Caller<'_, PluginContext>, ptr: i32, max_len: i32| -> wasmtime::Result<i32> {
            let mut execute_fetch = || -> RaiseResult<i32> {
                let data = caller.data().output_buffer.clone();
                let data_len = data.len();
                if data_len == 0 {
                    return Ok(0);
                }
                let mem = match get_memory(&mut caller) {
                    Some(m) => m,
                    None => raise_error!("ERR_WASM_MEMORY"),
                };
                let write_len = MinOf(data_len, max_len as usize);
                match mem.write(&mut caller, ptr as usize, &data[0..write_len]) {
                    Ok(_) => Ok(write_len as i32),
                    Err(err) => raise_error!("ERR_WASM_WRITE", error = err.to_string()),
                }
            };
            match execute_fetch() {
                Ok(res) => Ok(res),
                Err(e) => Err(wasmtime::Error::msg(e.to_string())),
            }
        },
    ) {
        raise_error!(
            "ERR_WASM_BINDING",
            error = e.to_string(),
            context = json_value!({"func": "host_fetch_result"})
        );
    }

    if let Err(e) = linker.func_wrap(
        "env",
        "host_signal_event",
        |mut caller: Caller<'_, PluginContext>, ptr: i32, len: i32| -> wasmtime::Result<i32> {
            let mut execute_signal = || -> RaiseResult<i32> {
                let req = match read_json_request(&mut caller, ptr, len) {
                    Ok(v) => v,
                    Err(err) => raise_error!("ERR_WASM_SIGNAL", error = err.to_string()),
                };
                caller.data_mut().signals.push(req);
                Ok(1)
            };
            match execute_signal() {
                Ok(res) => Ok(res),
                Err(e) => Err(wasmtime::Error::msg(e.to_string())),
            }
        },
    ) {
        raise_error!(
            "ERR_WASM_BINDING",
            error = e.to_string(),
            context = json_value!({"func": "host_signal_event"})
        );
    }

    // ========================================================================
    // 3. BASE DE DONNÉES (RÉSOLUE PAR MOUNT POINTS)
    // ========================================================================

    if let Err(e) = linker.func_wrap(
        "env",
        "host_db_read",
        |mut caller: Caller<'_, PluginContext>, ptr: i32, len: i32| -> wasmtime::Result<i32> {
            let mut execute_db_read = || -> RaiseResult<i32> {
                let req = match read_json_request(&mut caller, ptr, len) {
                    Ok(v) => v,
                    Err(err) => return Ok(error_to_buffer(&mut caller, &err.to_string())),
                };
                let col = req["collection"].as_str().unwrap_or("").to_string();
                let id = req["id"].as_str().unwrap_or("").to_string();

                // 🎯 Résilience : Utilisation des partitions dynamiques du PluginContext
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
                    Ok(None) => Ok(success_to_buffer(&mut caller, JsonValue::Null)),
                    Err(err) => Ok(error_to_buffer(&mut caller, &err.to_string())),
                }
            };
            match execute_db_read() {
                Ok(res) => Ok(res),
                Err(e) => Err(wasmtime::Error::msg(e.to_string())),
            }
        },
    ) {
        raise_error!(
            "ERR_WASM_BINDING",
            error = e.to_string(),
            context = json_value!({"func": "host_db_read"})
        );
    }

    if let Err(e) = linker.func_wrap(
        "env",
        "host_db_write",
        |mut caller: Caller<'_, PluginContext>, ptr: i32, len: i32| -> wasmtime::Result<i32> {
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
                    Ok(id) => Ok(success_to_buffer(
                        &mut caller,
                        json_value!({ "inserted_id": id }),
                    )),
                    Err(err) => Ok(error_to_buffer(&mut caller, &err.to_string())),
                }
            };
            match execute_db_write() {
                Ok(res) => Ok(res),
                Err(e) => Err(wasmtime::Error::msg(e.to_string())),
            }
        },
    ) {
        raise_error!(
            "ERR_WASM_BINDING",
            error = e.to_string(),
            context = json_value!({"func": "host_db_write"})
        );
    }

    // ========================================================================
    // 4. SERVICES ÉTENDUS (AI, MODEL, RULES)
    // ========================================================================

    if let Err(e) = linker.func_wrap(
        "env",
        "host_llm_inference",
        |mut caller: Caller<'_, PluginContext>, ptr: i32, len: i32| -> wasmtime::Result<i32> {
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
                    Some(orch_arc) => block_on(async move {
                        let mut orch = orch_arc.lock().await;
                        orch.ask(&prompt).await
                    }),
                    None => Err(crate::build_error!("ERR_COGNITIVE_PLUGIN_AI_OFFLINE")),
                };

                match response_result {
                    Ok(response) => Ok(success_to_buffer(
                        &mut caller,
                        json_value!({ "response": response }),
                    )),
                    Err(err) => Ok(error_to_buffer(&mut caller, &err.to_string())),
                }
            };
            match execute_llm() {
                Ok(res) => Ok(res),
                Err(e) => Err(wasmtime::Error::msg(e.to_string())),
            }
        },
    ) {
        raise_error!(
            "ERR_WASM_BINDING",
            error = e.to_string(),
            context = json_value!({"func": "host_llm_inference"})
        );
    }

    if let Err(e) = linker.func_wrap(
        "env",
        "host_model_query",
        |mut caller: Caller<'_, PluginContext>, ptr: i32, len: i32| -> wasmtime::Result<i32> {
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
                    let loader = ModelLoader::new_with_manager(mgr)?;
                    loader.get_element(&target_id).await
                });
                match result {
                    Ok(el) => Ok(success_to_buffer(&mut caller, json_value!(el))),
                    Err(err) => Ok(error_to_buffer(&mut caller, &err.to_string())),
                }
            };
            match execute_model_query() {
                Ok(res) => Ok(res),
                Err(e) => Err(wasmtime::Error::msg(e.to_string())),
            }
        },
    ) {
        raise_error!(
            "ERR_WASM_BINDING",
            error = e.to_string(),
            context = json_value!({"func": "host_model_query"})
        );
    }

    if let Err(e) = linker.func_wrap(
        "env",
        "host_rule_validate",
        |mut caller: Caller<'_, PluginContext>, ptr: i32, len: i32| -> wasmtime::Result<i32> {
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
                    match store.sync_from_db().await {
                        Ok(_) => (),
                        Err(sync_err) => {
                            raise_error!("ERR_RULE_SYNC_FAILED", error = sync_err.to_string())
                        }
                    }
                    let rules = if target_filter.is_empty() {
                        store.get_all_rules()
                    } else {
                        store.get_rules_for_target(&target_filter)
                    };
                    Ok(rules)
                });
                match result {
                    Ok(rules) => Ok(success_to_buffer(&mut caller, json_value!(rules))),
                    Err(err) => Ok(error_to_buffer(&mut caller, &err.to_string())),
                }
            };
            match execute_rule_validate() {
                Ok(res) => Ok(res),
                Err(e) => Err(wasmtime::Error::msg(e.to_string())),
            }
        },
    ) {
        raise_error!(
            "ERR_WASM_BINDING",
            error = e.to_string(),
            context = json_value!({"func": "host_rule_validate"})
        );
    }

    Ok(())
}

// --- HELPERS (STRICTS) ---

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
        None => raise_error!("ERR_WASM_MEMORY_OUT_OF_BOUNDS"),
    };
    match String::from_utf8(data.to_vec()) {
        Ok(s) => Ok(s),
        Err(e) => raise_error!("ERR_WASM_UTF8_DECODE_FAILED", error = e.to_string()),
    }
}

fn read_json_request(
    caller: &mut Caller<'_, PluginContext>,
    ptr: i32,
    len: i32,
) -> RaiseResult<JsonValue> {
    let mem = match get_memory(caller) {
        Some(m) => m,
        None => raise_error!("ERR_WASM_NO_MEMORY"),
    };
    let json_str = read_string_from_wasm(caller, &mem, ptr, len)?;
    match json::deserialize_from_str(&json_str) {
        Ok(v) => Ok(v),
        Err(e) => raise_error!("ERR_JSON_PARSE_FAILED", error = e.to_string()),
    }
}

fn success_to_buffer(caller: &mut Caller<'_, PluginContext>, data: JsonValue) -> i32 {
    let json_bytes = data.to_string().into_bytes();
    let len = json_bytes.len() as i32;
    caller.data_mut().output_buffer = json_bytes;
    len
}

fn error_to_buffer(caller: &mut Caller<'_, PluginContext>, msg: &str) -> i32 {
    let json_bytes = json_value!({ "error": msg }).to_string().into_bytes();
    let len = json_bytes.len() as i32;
    caller.data_mut().output_buffer = json_bytes;
    len
}

// =========================================================================
// TESTS UNITAIRES (CONFORMITÉ STRICTE)
// =========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json_db::storage::{JsonDbConfig, StorageEngine};
    use crate::utils::testing::AgentDbSandbox;
    use tempfile::tempdir;
    use wasmtime::Engine;

    #[test]
    fn test_register_functions_integrity() -> RaiseResult<()> {
        let engine = Engine::default();
        let mut linker = Linker::new(&engine);
        let temp_dir = tempdir().unwrap();
        let config = JsonDbConfig::new(temp_dir.path().to_path_buf());
        let storage = StorageEngine::new(config)?;

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

        Ok(())
    }

    /// 🎯 NOUVEAU TEST : Résilience de la résolution des partitions
    #[async_test]
    async fn test_cognitive_mount_point_integrity() -> RaiseResult<()> {
        let _sandbox = AgentDbSandbox::new().await?;
        let config = AppConfig::get();
        // Vérifie que les points de montage système sont accessibles
        assert!(
            !config.mount_points.system.domain.is_empty(),
            "Partition système non résolue"
        );

        Ok(())
    }
}
