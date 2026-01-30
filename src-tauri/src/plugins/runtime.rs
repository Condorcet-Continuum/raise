// FICHIER : src-tauri/src/plugins/runtime.rs

use super::cognitive;
use crate::ai::orchestrator::AiOrchestrator;
use crate::json_db::storage::StorageEngine;
use anyhow::{anyhow, Result};
use serde_json::Value;
use std::sync::{Arc, Mutex};
use wasmtime::*;

/// Le contexte partagé accessible par toutes les "Host Functions".
/// C'est ici que réside l'intelligence du Hub et les contrôles de gouvernance.
pub struct PluginContext {
    // --- Stockage & Isolation ---
    pub storage: StorageEngine,
    pub space: String,
    pub db: String,

    // --- Services Backend Connectés (Hub) ---
    pub ai_orchestrator: Option<Arc<Mutex<AiOrchestrator>>>,

    // --- Gouvernance & Communication (Workflow Integration) ---
    /// Le Mandat : Définit ce que le plugin a le droit de faire (ex: lecture seule).
    pub mandate: Option<Value>,

    /// Les Signaux : Événements structurés émis par le plugin vers le WorkflowExecutor.
    pub signals: Vec<Value>,

    // --- Gestion de la Mémoire (Mailbox) ---
    pub output_buffer: Vec<u8>,
}

pub struct CognitivePlugin {
    store: Store<PluginContext>,
    instance: Instance,
}

impl CognitivePlugin {
    /// Crée une nouvelle instance de plugin cognitif.
    pub fn new(
        binary: &[u8],
        storage: &StorageEngine,
        space: &str,
        db: &str,
        ai: Option<Arc<Mutex<AiOrchestrator>>>,
    ) -> Result<Self> {
        let engine = Engine::default();
        let mut linker = Linker::new(&engine);

        // --- 1. Enregistrement des capacités cognitives ---
        cognitive::register_host_functions(&mut linker)?;

        // --- 2. Initialisation du Context ---
        let ctx = PluginContext {
            storage: storage.clone(),
            space: space.to_string(),
            db: db.to_string(),
            ai_orchestrator: ai,
            mandate: None,
            signals: Vec::new(),
            output_buffer: Vec::new(),
        };

        let mut store = Store::new(&engine, ctx);
        let module = Module::new(&engine, binary)?;
        let instance = linker.instantiate(&mut store, &module)?;

        Ok(Self { store, instance })
    }

    /// Injecte un Mandat de gouvernance avant l'exécution.
    pub fn set_mandate(&mut self, mandate: Value) {
        self.store.data_mut().mandate = Some(mandate);
    }

    /// Récupère les signaux (événements) émis par le plugin après son exécution.
    pub fn get_signals(&self) -> Vec<Value> {
        self.store.data().signals.clone()
    }

    /// Exécute le point d'entrée "run" du plugin.
    pub fn run(&mut self) -> Result<i32> {
        let run_func = self
            .instance
            .get_typed_func::<(), i32>(&mut self.store, "run")
            .map_err(|_| anyhow!("Fonction 'run' introuvable dans le plugin"))?;

        let result = run_func.call(&mut self.store, ())?;
        Ok(result)
    }
}

// ============================================================================
// TESTS UNITAIRES
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json_db::storage::{JsonDbConfig, StorageEngine};
    use serde_json::json;
    use tempfile::tempdir;

    fn create_dummy_wasm() -> Vec<u8> {
        vec![
            0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00, 0x01, 0x05, 0x01, 0x60, 0x00, 0x01,
            0x7f, 0x03, 0x02, 0x01, 0x00, 0x07, 0x07, 0x01, 0x03, 0x72, 0x75, 0x6e, 0x00, 0x00,
            0x0a, 0x06, 0x01, 0x04, 0x00, 0x41, 0x2a, 0x0b,
        ]
    }

    #[test]
    fn test_mandate_injection_and_signals() {
        let dir = tempdir().unwrap();
        let config = JsonDbConfig::new(dir.path().to_path_buf());
        let storage = StorageEngine::new(config);
        let wasm = create_dummy_wasm();

        let mut plugin = CognitivePlugin::new(&wasm, &storage, "space", "db", None).unwrap();

        // 1. Test Mandat
        let test_mandate = json!({ "permissions": { "readonly": true } });
        plugin.set_mandate(test_mandate.clone());
        assert_eq!(plugin.store.data().mandate, Some(test_mandate));

        // 2. Test Signaux (Vérification de l'initialisation vide)
        let signals = plugin.get_signals();
        assert!(signals.is_empty());
    }
}
