use super::runtime::CognitivePlugin;
use crate::json_db::storage::StorageEngine;
use anyhow::{anyhow, Result};
use std::collections::HashMap;
use std::fs;
use std::sync::{Arc, Mutex};

pub struct PluginManager {
    storage: StorageEngine,
    // Stockage des instances actives
    plugins: Arc<Mutex<HashMap<String, CognitivePlugin>>>,
}

impl PluginManager {
    pub fn new(storage: &StorageEngine) -> Self {
        Self {
            storage: storage.clone(),
            plugins: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Charge un plugin depuis le disque
    pub fn load_plugin(
        &self,
        plugin_id: &str,
        file_path: &str,
        space: &str,
        db: &str,
    ) -> Result<()> {
        println!("🔌 Chargement du plugin : {} ({})", plugin_id, file_path);

        let binary = fs::read(file_path)
            .map_err(|e| anyhow!("Impossible de lire le fichier wasm : {}", e))?;

        let plugin = CognitivePlugin::new(&binary, &self.storage, space, db)?;

        self.plugins
            .lock()
            .unwrap()
            .insert(plugin_id.to_string(), plugin);
        Ok(())
    }

    /// Exécute un plugin chargé
    pub fn run_plugin(&self, plugin_id: &str) -> Result<i32> {
        let mut map = self.plugins.lock().unwrap();
        if let Some(plugin) = map.get_mut(plugin_id) {
            println!("▶️ Exécution du plugin : {}", plugin_id);
            plugin.run()
        } else {
            Err(anyhow!("Plugin introuvable : {}", plugin_id))
        }
    }

    /// Liste les plugins chargés
    pub fn list_active_plugins(&self) -> Vec<String> {
        self.plugins.lock().unwrap().keys().cloned().collect()
    }
}
