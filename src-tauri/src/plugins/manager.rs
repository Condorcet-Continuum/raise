// FICHIER : src-tauri/src/plugins/manager.rs
use crate::utils::prelude::*;

use super::runtime::CognitivePlugin;
use crate::ai::orchestrator::AiOrchestrator;
use crate::json_db::storage::StorageEngine;

pub struct PluginManager {
    storage: StorageEngine,
    ai_orchestrator: Option<SharedRef<AsyncMutex<AiOrchestrator>>>,
    plugins: SharedRef<AsyncMutex<UnorderedMap<String, CognitivePlugin>>>,
}

impl PluginManager {
    pub fn new(
        storage: &StorageEngine,
        ai_orchestrator: Option<SharedRef<AsyncMutex<AiOrchestrator>>>,
    ) -> Self {
        Self {
            storage: storage.clone(),
            ai_orchestrator,
            plugins: SharedRef::new(AsyncMutex::new(UnorderedMap::new())),
        }
    }

    pub async fn load_plugin(
        &self,
        plugin_id: &str,
        file_path: &str,
        space: &str,
        db: &str,
    ) -> RaiseResult<()> {
        println!("🔌 Chargement du plugin : {} ({})", plugin_id, file_path);

        let binary = match fs::read_async(file_path).await {
            Ok(b) => b,
            Err(e) => {
                raise_error!(
                    "ERR_PLUGIN_WASM_READ_FAIL",
                    error = format!("Impossible de lire le fichier binaire WebAssembly : {}", e),
                    context = json_value!({
                        "file_path": file_path,
                        "os_error": e.to_string(),
                        "action": "load_wasm_binary",
                        "hint": "Vérifiez que le fichier .wasm existe au chemin indiqué et que l'application dispose des droits de lecture."
                    })
                );
            }
        };

        let plugin = CognitivePlugin::new(
            &binary,
            &self.storage,
            space,
            db,
            self.ai_orchestrator.clone(),
        )?;

        self.plugins
            .lock()
            .await
            .insert(plugin_id.to_string(), plugin);
        Ok(())
    }

    pub async fn run_plugin_with_context(
        &self,
        plugin_id: &str,
        mandate: Option<JsonValue>,
    ) -> RaiseResult<(i32, Vec<JsonValue>)> {
        let mut map = self.plugins.lock().await;
        if let Some(plugin) = map.get_mut(plugin_id) {
            if let Some(m) = mandate {
                plugin.set_mandate(m);
            }

            let result = plugin.run()?;
            let signals = plugin.get_signals();

            Ok((result, signals))
        } else {
            raise_error!(
                "ERR_PLUGIN_REGISTRY_NOT_FOUND",
                error = format!("Instance de plugin introuvable : {}", plugin_id),
                context = json_value!({
                    "requested_plugin_id": plugin_id,
                    "action": "resolve_plugin_instance",
                    "hint": "Vérifiez que le plugin est bien activé dans votre fichier de configuration et qu'il a été compilé sans erreur."
                })
            );
        }
    }

    pub async fn list_active_plugins(&self) -> Vec<String> {
        self.plugins.lock().await.keys().cloned().collect()
    }
}

// ============================================================================
// TESTS UNITAIRES
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json_db::storage::{JsonDbConfig, StorageEngine};

    fn create_test_env() -> (PluginManager, StorageEngine, tempfile::TempDir) {
        let dir = tempdir().unwrap();
        let config = JsonDbConfig::new(dir.path().to_path_buf());
        let storage = StorageEngine::new(config);
        let manager = PluginManager::new(&storage, None);
        (manager, storage, dir)
    }

    /// Générateur de Bytecode WASM ultra-sécurisé (Validé par spécification).
    /// Contient uniquement une fonction exportée "run" retournant 1.
    /// Aucune dépendance de section Data ou Import pour éviter les erreurs de parsing.
    fn generate_minimal_wasm() -> Vec<u8> {
        vec![
            0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00, // Magic + Version
            0x01, 0x05, 0x01, 0x60, 0x00, 0x01, 0x7f, // Type: () -> i32
            0x03, 0x02, 0x01, 0x00, // Function: utilise type 0
            0x07, 0x07, 0x01, 0x03, 0x72, 0x75, 0x6e, 0x00, 0x00, // Export: "run"
            0x0a, 0x06, 0x01, 0x04, 0x00, 0x41, 0x01, 0x0b, // Code: i32.const 1, end
        ]
    }

    #[async_test]
    async fn test_plugin_workflow_signal_retrieval() {
        let (manager, _storage, _tmp_dir) = create_test_env();

        let wasm_bytes = generate_minimal_wasm();
        let wasm_path = _tmp_dir.path().join("workflow_spy.wasm");
        fs::write_async(&wasm_path, wasm_bytes).await.unwrap();

        manager
            .load_plugin("workflow_spy", wasm_path.to_str().unwrap(), "s", "d")
            .await
            .expect("Le chargement a échoué");

        // Test d'exécution avec injection de mandat (même si le binaire minimal l'ignore)
        let mandate = json_value!({ "id": "test_mandate" });
        let (result_code, signals) = manager
            .run_plugin_with_context("workflow_spy", Some(mandate))
            .await
            .expect("L'exécution a échoué");

        assert_eq!(result_code, 1, "Le plugin minimal doit retourner 1");
        assert!(
            signals.is_empty(),
            "Les signaux doivent être vides pour ce binaire minimal"
        );

        println!("✅ Test de cycle de vie Manager passé avec succès.");
    }

    #[async_test]
    async fn test_plugin_not_found() {
        let (manager, _, _) = create_test_env();
        let res = manager.run_plugin_with_context("unknown", None).await;
        assert!(res.is_err());
    }
}
