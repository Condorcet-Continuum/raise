// FICHIER : src-tauri/src/workflow_engine/wasm_host.rs
use crate::utils::{prelude::*, Result};

use wasmtime::*;

/// Wrapper autour du Runtime Wasmtime
pub struct WasmHost {
    engine: Engine,
}

impl WasmHost {
    /// Initialise le moteur WebAssembly (Wasmtime)
    pub fn new() -> Result<Self> {
        let config = Config::new();
        // Conversion explicite de l'erreur anyhow::Error vers String
        let engine = Engine::new(&config).map_err(|e| e.to_string())?;
        Ok(Self { engine })
    }

    /// Exécute un binaire WASM avec un input JSON
    /// Le module doit exposer :
    /// - alloc(size: i32) -> i32 (pointeur)
    /// - memory (Linear Memory)
    /// - process(ptr: i32, len: i32) -> i64 (packed ptr/len)
    pub fn run_module(&self, wasm_bytes: &[u8], input: &Value) -> Result<Value> {
        let mut store = Store::new(&self.engine, ());

        // Compilation du module
        let module = Module::new(&self.engine, wasm_bytes).map_err(|e| e.to_string())?;

        // Linker (pour lier des fonctions hôtes si nécessaire plus tard)
        let linker = Linker::new(&self.engine);

        // Instanciation
        let instance = linker
            .instantiate(&mut store, &module)
            .map_err(|e| e.to_string())?;

        // 1. Allocation mémoire pour l'input dans l'invité
        let input_str = input.to_string();
        let input_bytes = input_str.as_bytes();

        // Récupération de la fonction 'alloc'
        let alloc_fn = instance
            .get_typed_func::<i32, i32>(&mut store, "alloc")
            .map_err(|_| "Le module WASM doit exporter la fonction 'alloc(size)'".to_string())?;

        // Appel de l'allocation
        let ptr = alloc_fn
            .call(&mut store, input_bytes.len() as i32)
            .map_err(|e| format!("Erreur lors de l'allocation : {}", e))?;

        // 2. Écriture des données dans la mémoire partagée
        let memory = instance
            .get_memory(&mut store, "memory")
            .ok_or_else(|| "Le module WASM doit exporter 'memory'".to_string())?;

        memory
            .write(&mut store, ptr as usize, input_bytes)
            .map_err(|e| format!("Erreur d'écriture mémoire : {}", e))?;

        // 3. Exécution de la logique (fonction 'process')
        let process_fn = instance
            .get_typed_func::<(i32, i32), i64>(&mut store, "process")
            .map_err(|_| {
                "Le module WASM doit exporter la fonction 'process(ptr, len)'".to_string()
            })?;

        let packed_result = process_fn
            .call(&mut store, (ptr, input_bytes.len() as i32))
            .map_err(|e| format!("Erreur d'exécution du process : {}", e))?;

        // 4. Récupération du résultat (Pointeur + Longueur paquets dans un i64)
        // High 32 bits = Ptr, Low 32 bits = Len
        let res_ptr = (packed_result >> 32) as usize;
        let res_len = (packed_result & 0xFFFFFFFF) as usize;

        let mut buffer = vec![0u8; res_len];
        memory
            .read(&store, res_ptr, &mut buffer)
            .map_err(|e| format!("Erreur de lecture du résultat : {}", e))?;

        let output_str = String::from_utf8(buffer)
            .map_err(|e| format!("Résultat WASM invalide (UTF-8) : {}", e))?;

        // Parsing final JSON
        let result = serde_json::from_str(&output_str)
            .map_err(|e| format!("Résultat WASM n'est pas un JSON valide : {}", e))?;

        Ok(result)
    }
}

// --- TESTS UNITAIRES ---
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_initialization() {
        let host = WasmHost::new();
        assert!(host.is_ok(), "L'initialisation de Wasmtime devrait réussir");
    }

    #[test]
    fn test_run_invalid_bytes() {
        let host = WasmHost::new().unwrap();
        // Séquence d'octets aléatoire (pas un header WASM valide)
        let bad_bytes = vec![0, 1, 2, 3, 4, 5];
        let input = json!({});

        let result = host.run_module(&bad_bytes, &input);

        // On s'assure juste que ça plante proprement
        assert!(
            result.is_err(),
            "Le module aurait dû échouer au chargement (bytes invalides)"
        );
        println!("DEBUG [Invalid Bytes Error]: {:?}", result.err());
    }

    #[test]
    fn test_missing_exports() {
        let host = WasmHost::new().unwrap();

        // Header WASM valide minimal (Module vide qui ne fait rien) : \0asm\1\0\0\0
        let empty_module_bytes = vec![
            0x00, 0x61, 0x73, 0x6D, // \0asm
            0x01, 0x00, 0x00, 0x00, // Version 1
        ];
        let input = json!({});

        let result = host.run_module(&empty_module_bytes, &input);

        assert!(
            result.is_err(),
            "Le module aurait dû échouer car il manque les fonctions alloc/process"
        );
        let err_msg = result.err().unwrap().to_string();
        println!("DEBUG [Missing Exports Error]: {}", err_msg);

        assert!(
            err_msg.contains("alloc") || err_msg.contains("export"),
            "Message d'erreur inattendu : {}",
            err_msg
        );
    }
}
