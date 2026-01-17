// FICHIER : src-tauri/src/plugins/cognitive.rs

use super::runtime::PluginContext;
use crate::json_db::collections::manager::CollectionsManager;
use anyhow::Result;
use futures::executor::block_on;
use serde_json::Value;
use wasmtime::{Caller, Extern, Linker};

/// Enregistre les fonctions DB dans le linker WASM
pub fn register_host_functions(linker: &mut Linker<PluginContext>) -> Result<()> {
    // FONCTION : host_db_read(ptr, len) -> 1 (succès) / 0 (échec)
    // Le plugin envoie une requête JSON, l'hôte l'exécute et affiche le résultat (pour l'instant)
    linker.func_wrap(
        "env",
        "host_db_read",
        |mut caller: Caller<'_, PluginContext>, ptr: i32, len: i32| -> i32 {
            // 1. Lire la mémoire du WASM pour récupérer la requête
            let mem = match caller.get_export("memory") {
                Some(Extern::Memory(m)) => m,
                _ => return 0,
            };

            let request_str = match read_string_from_wasm(&mut caller, &mem, ptr, len) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("🔥 [WASM Error] Memory access: {}", e);
                    return 0;
                }
            };

            // 2. Interpréter la requête (ex: { "collection": "users", "id": "admin" })
            println!("🧠 [Cognitive Bridge] Requête reçue : {}", request_str);

            let response = match serde_json::from_str::<Value>(&request_str) {
                Ok(req) => {
                    // Accès sécurisé au contexte (Storage)
                    let ctx = caller.data();
                    let mgr = CollectionsManager::new(&ctx.storage, &ctx.space, &ctx.db);

                    let col = req["collection"].as_str().unwrap_or("");
                    let id = req["id"].as_str().unwrap_or("");

                    // CORRECTION : mgr.get() renvoie une Future. block_on permet de récupérer
                    // le résultat de manière synchrone pour le Linker WASM.
                    match block_on(mgr.get(col, id)) {
                        Ok(Some(doc)) => doc.to_string(),
                        Ok(None) => String::from("null"),
                        Err(e) => format!("{{ \"error\": \"{}\" }}", e),
                    }
                }
                Err(_) => String::from("{ \"error\": \"Invalid JSON\" }"),
            };

            println!("🧠 [Cognitive Bridge] Réponse générée : {}", response);

            // TODO: Pour un système complet, il faudrait écrire 'response' dans la mémoire du WASM
            // via une fonction d'allocation exportée par le plugin (ex: 'malloc').
            // Pour l'instant, on considère que l'action est faite côté Host.

            1 // Succès
        },
    )?;

    // FONCTION : plugin_log(ptr, len)
    linker.func_wrap(
        "env",
        "plugin_log",
        |mut caller: Caller<'_, PluginContext>, ptr: i32, len: i32| -> i32 {
            let mem = match caller.get_export("memory") {
                Some(Extern::Memory(m)) => m,
                _ => return -1,
            };

            if let Ok(msg) = read_string_from_wasm(&mut caller, &mem, ptr, len) {
                println!("🤖 [PLUGIN LOG]: {}", msg);
            }

            // CORRECTIF E0308 : On ajoute un retour explicite car la closure attend un i32
            0
        },
    )?;

    Ok(())
}

/// Helper pour extraire une String de la mémoire linéaire du WASM
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
    Ok(String::from_utf8(data.to_vec())?)
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
            wasi_out_buffer: Vec::new(),
        };

        // Vérification que les fonctions d'hôte s'enregistrent sans erreur
        let result = register_host_functions(&mut linker);
        assert!(result.is_ok());
    }
}
