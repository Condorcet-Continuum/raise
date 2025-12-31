use super::cognitive;
use crate::json_db::storage::StorageEngine;
use anyhow::{anyhow, Result};
use wasmtime::*;

/// Le contexte partagé accessible par toutes les "Host Functions"
pub struct PluginContext {
    pub storage: StorageEngine,
    pub space: String,
    pub db: String,
    // Mémoire tampon pour les échanges complexes (optionnel pour la V1)
    pub wasi_out_buffer: Vec<u8>,
}

pub struct CognitivePlugin {
    store: Store<PluginContext>,
    instance: Instance,
}

impl CognitivePlugin {
    pub fn new(binary: &[u8], storage: &StorageEngine, space: &str, db: &str) -> Result<Self> {
        let engine = Engine::default();
        let mut linker = Linker::new(&engine);

        // --- 1. Enregistrement des capacités cognitives ---
        // On passe le linker à cognitive.rs pour qu'il y attache "db_read", "db_write", etc.
        cognitive::register_host_functions(&mut linker)?;

        // --- 2. Initialisation du Store ---
        let ctx = PluginContext {
            storage: storage.clone(),
            space: space.to_string(),
            db: db.to_string(),
            wasi_out_buffer: Vec::new(),
        };

        let mut store = Store::new(&engine, ctx);
        let module = Module::new(&engine, binary)?;
        let instance = linker.instantiate(&mut store, &module)?;

        Ok(Self { store, instance })
    }

    /// Exécute le point d'entrée du plugin (la fonction "run")
    /// Pour simplifier, on imagine que le plugin ne prend pas d'arg et retourne un status
    pub fn run(&mut self) -> Result<i32> {
        // On cherche la fonction exportée "run" ou "_start"
        let run_func = self
            .instance
            .get_typed_func::<(), i32>(&mut self.store, "run")
            .map_err(|_| anyhow!("Fonction 'run' introuvable dans le module WASM"))?;

        let result = run_func.call(&mut self.store, ())?;
        Ok(result)
    }
}
