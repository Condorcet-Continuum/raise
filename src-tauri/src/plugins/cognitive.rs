use anyhow::{Context, Result};
use serde::Serialize;
use std::fs;
use std::path::Path;
use wasmtime::*;

pub struct CognitiveManager {
    engine: Engine,
}

impl Default for CognitiveManager {
    fn default() -> Self {
        Self::new()
    }
}

impl CognitiveManager {
    pub fn new() -> Self {
        let engine = Engine::default();
        Self { engine }
    }

    pub fn run_block<T: Serialize>(&self, plugin_path: &Path, input_data: &T) -> Result<String> {
        // 1. Lecture fichier
        let wasm_bytes = fs::read(plugin_path)
            .with_context(|| format!("Plugin introuvable : {:?}", plugin_path))?;

        // 2. Setup Wasmtime
        let module = Module::new(&self.engine, &wasm_bytes)?;
        let mut store = Store::new(&self.engine, ());
        let instance = Instance::new(&mut store, &module, &[])?;

        // 3. Bindings (alloc/run)
        let alloc_fn = instance.get_typed_func::<i32, i32>(&mut store, "alloc")?;
        let run_fn = instance.get_typed_func::<(i32, i32), i64>(&mut store, "run_analysis")?;
        let memory = instance
            .get_memory(&mut store, "memory")
            .context("Mémoire WASM non exportée")?;

        // 4. Input -> WASM
        let input_json = serde_json::to_string(input_data)?;
        let input_bytes = input_json.as_bytes();
        let input_len = input_bytes.len() as i32;
        let input_ptr = alloc_fn.call(&mut store, input_len)?;
        memory.write(&mut store, input_ptr as usize, input_bytes)?;

        // 5. Exécution
        let packed_result = run_fn.call(&mut store, (input_ptr, input_len))?;

        // 6. Output -> Host
        let result_ptr = (packed_result >> 32) as usize;
        let result_len = (packed_result & 0xFFFFFFFF) as usize;
        let mut buffer = vec![0u8; result_len];
        memory.read(&mut store, result_ptr, &mut buffer)?;

        Ok(String::from_utf8(buffer)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::path::PathBuf;

    // Structure locale minimaliste pour simuler CognitiveModel
    // (On pourrait importer genaptitude-core-api si on l'ajoutait aux dev-dependencies,
    // mais ici on utilise serde_json::Value pour rester souple)
    #[derive(Serialize)]
    struct MockModel {
        id: String,
        elements: HashMap<String, MockElement>,
        metadata: HashMap<String, String>,
    }

    #[derive(Serialize)]
    struct MockElement {
        name: String,
        kind: String,
        properties: HashMap<String, String>,
    }

    #[test]
    fn test_integration_load_and_run_wasm() {
        // 1. Localiser le fichier WASM compilé
        // On remonte de src-tauri/src/plugins/ vers la racine du projet
        let project_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .to_path_buf();
        let wasm_path = project_root.join("wasm-modules/analyzers/consistency_basic.wasm");

        // Si le fichier n'existe pas, on ignore le test (pour éviter de casser le CI si pas buildé)
        if !wasm_path.exists() {
            println!("⚠️ Test ignoré : Le fichier WASM n'est pas présent à {:?}. Lancez ./scripts/build_plugins.sh d'abord.", wasm_path);
            return;
        }

        // 2. Préparer les données
        let mut elements = HashMap::new();
        elements.insert(
            "elt-1".to_string(),
            MockElement {
                name: "Test Integration".to_string(),
                kind: "Unit".to_string(),
                properties: HashMap::new(),
            },
        );

        let input_data = MockModel {
            id: "integration-test".to_string(),
            elements,
            metadata: HashMap::new(),
        };

        // 3. Lancer le manager
        let manager = CognitiveManager::new();
        let result = manager.run_block(&wasm_path, &input_data);

        // 4. Vérification
        assert!(
            result.is_ok(),
            "L'exécution du WASM a échoué : {:?}",
            result.err()
        );

        let output_json = result.unwrap();
        println!("Sortie du WASM : {}", output_json);

        assert!(
            output_json.contains("fr.genaptitude.blocks.consistency"),
            "L'ID du bloc est incorrect"
        );
        assert!(
            output_json.contains("Success"),
            "Le statut devrait être Success"
        );
    }
}
