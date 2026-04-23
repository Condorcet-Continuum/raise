// FICHIER : src-tauri/src/ai/world_model/engine.rs

use crate::json_db::collections::manager::CollectionsManager;
use crate::utils::prelude::*; // 🎯 Façade Unique RAISE

use crate::ai::nlp::parser::CommandType;
use crate::ai::world_model::dynamics::WorldModelPredictor;
use crate::ai::world_model::perception::ArcadiaEncoder;
use crate::ai::world_model::representation::VectorQuantizer;
use crate::model_engine::types::ArcadiaElement;

pub struct WorldAction {
    pub intent: CommandType,
}

impl WorldAction {
    /// Convertit une intention sémantique en tenseur "One-Hot" pour le prédicteur.
    pub fn to_tensor(&self, dim: usize) -> RaiseResult<NeuralTensor> {
        let mut data = vec![0f32; dim];
        let idx = match self.intent {
            CommandType::Create => 0,
            CommandType::Delete => 1,
            CommandType::Search => 2,
            CommandType::Explain => 3,
            CommandType::Unknown => 4,
        };

        if idx < dim {
            data[idx] = 1.0;
        }

        // 🎯 Pattern Match strict pour la création tensorielle
        match NeuralTensor::from_vec(data, (1, dim), &ComputeHardware::Cpu) {
            Ok(t) => Ok(t),
            Err(e) => raise_error!(
                "ERR_TENSOR_FROM_VEC",
                error = e.to_string(),
                context = json_value!({ "action": "create_action_tensor", "dim": dim })
            ),
        }
    }
}

pub struct NeuroSymbolicEngine {
    pub varmap: NeuralWeightsMap,
    pub quantizer: VectorQuantizer,
    pub predictor: WorldModelPredictor,
    pub config: WorldModelConfig,
}

impl NeuroSymbolicEngine {
    /// Initialise le moteur Neuro-Symbolique Arcadia.
    pub fn new(config: WorldModelConfig, varmap: NeuralWeightsMap) -> RaiseResult<Self> {
        let vb =
            NeuralWeightsBuilder::from_varmap(&varmap, ComputeType::F32, &ComputeHardware::Cpu);

        let quantizer = match VectorQuantizer::new(&config, vb.pp("quantizer")) {
            Ok(q) => q,
            Err(e) => raise_error!("ERR_WM_QUANTIZER_INIT", error = e.to_string()),
        };

        let predictor = match WorldModelPredictor::new(&config, vb.pp("dynamics")) {
            Ok(p) => p,
            Err(e) => raise_error!("ERR_WM_PREDICTOR_INIT", error = e.to_string()),
        };

        Ok(Self {
            varmap,
            quantizer,
            predictor,
            config,
        })
    }

    pub fn new_empty(config: WorldModelConfig) -> RaiseResult<Self> {
        let varmap = NeuralWeightsMap::new();
        Self::new(config, varmap)
    }

    /// Simule l'évolution de l'état du monde Arcadia face à une action.
    pub fn simulate(
        &self,
        element: &ArcadiaElement,
        action: WorldAction,
    ) -> RaiseResult<NeuralTensor> {
        let raw_perception = ArcadiaEncoder::encode_element(element)?;
        let token = self.quantizer.tokenize(&raw_perception)?;
        let state_quantized = self.quantizer.decode(&token)?;
        let action_tensor = action.to_tensor(self.config.action_dim)?;

        match self.predictor.forward(&state_quantized, &action_tensor) {
            Ok(future) => Ok(future),
            Err(e) => raise_error!("ERR_WM_FORWARD_PASS", error = e.to_string()),
        }
    }

    fn extract_tensors_sync(&self) -> RaiseResult<UnorderedMap<String, NeuralTensor>> {
        let data_guard = match self.varmap.data().lock() {
            Ok(guard) => guard,
            Err(_) => raise_error!("ERR_LOCK_PANIC", error = "Varmap lock poisoned"),
        };
        let mut extracted = UnorderedMap::new();
        for (k, v) in data_guard.iter() {
            extracted.insert(k.clone(), v.as_tensor().clone());
        }
        Ok(extracted)
    }

    /// 🎯 RÉSOLUTION DYNAMIQUE : Localisation du modèle via Mount Points
    fn get_model_dir(manager: &CollectionsManager<'_>) -> PathBuf {
        manager
            .storage
            .config
            .db_root(&manager.space, &manager.db)
            .join("tensors")
            .join("world_model")
    }

    /// Sauvegarde les poids du modèle de manière asynchrone et résiliente.
    pub async fn save(&self, manager: &CollectionsManager<'_>) -> RaiseResult<()> {
        let model_dir = Self::get_model_dir(manager);
        fs::ensure_dir_async(&model_dir).await?;

        let path = model_dir.join("world_model.safetensors");
        let tensors = self.extract_tensors_sync()?;

        let path_display = path.to_string_lossy().to_string();

        // 🎯 Pattern Match strict sur le spawn (Zéro Dette)
        let spawn_result = match spawn_cpu_task(move || SafeTensorsIO::save(&tensors, path)).await {
            Ok(res) => res,
            Err(e) => raise_error!(
                "ERR_ASYNC_SPAWN_FAILURE",
                error = e.to_string(),
                context = json_value!({ "path": path_display })
            ),
        };

        match spawn_result {
            Ok(_) => Ok(()),
            Err(e) => raise_error!(
                "ERR_MODEL_SAVE_SAFETENSORS",
                error = e.to_string(),
                context = json_value!({ "path": path_display })
            ),
        }
    }

    /// Charge les poids du modèle depuis le disque avec validation de format.
    pub async fn load(
        manager: &CollectionsManager<'_>,
        config: WorldModelConfig,
    ) -> RaiseResult<Self> {
        let model_dir = Self::get_model_dir(manager);
        let path = model_dir.join("world_model.safetensors");

        if !fs::exists_async(&path).await {
            raise_error!(
                "ERR_MODEL_NOT_FOUND",
                error = "Fichier World Model introuvable.",
                context = json_value!({ "path": path.to_string_lossy() })
            );
        }

        let buffer = fs::read_async(path).await?;

        let tensors = match SafeTensorsIO::load_buffer(&buffer, &ComputeHardware::Cpu) {
            Ok(t) => t,
            Err(e) => raise_error!(
                "ERR_MODEL_LOAD_BUFFER",
                error = e.to_string(),
                context = json_value!({ "buffer_size": buffer.len() })
            ),
        };

        let varmap = NeuralWeightsMap::new();
        {
            let mut data = match varmap.data().lock() {
                Ok(guard) => guard,
                Err(_) => raise_error!("ERR_LOCK_POISONED", error = "Varmap load lock error"),
            };
            for (name, tensor) in tensors {
                let var = match NeuralVar::from_tensor(&tensor) {
                    Ok(v) => v,
                    Err(e) => raise_error!(
                        "ERR_MODEL_VAR_CONVERSION",
                        error = e.to_string(),
                        context = json_value!({"name": name})
                    ),
                };
                data.insert(name, var);
            }
        }

        Self::new(config, varmap)
    }

    pub async fn exists(manager: &CollectionsManager<'_>) -> bool {
        let model_dir = Self::get_model_dir(manager);
        let path = model_dir.join("world_model.safetensors");
        fs::exists_async(&path).await
    }
}

// =========================================================================
// TESTS (Validation Topologique Arcadia & Résilience)
// =========================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use crate::model_engine::types::NameType;
    use crate::utils::testing::AgentDbSandbox;

    fn get_test_config() -> WorldModelConfig {
        WorldModelConfig {
            vocab_size: 10,
            embedding_dim: 16,
            action_dim: 5,
            hidden_dim: 32,
            use_gpu: false,
        }
    }

    #[test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    fn test_engine_simulation_flow() -> RaiseResult<()> {
        let varmap = NeuralWeightsMap::new();
        let config = get_test_config();
        let engine = NeuroSymbolicEngine::new(config, varmap)?;

        let element = ArcadiaElement {
            id: "1".to_string(),
            name: NameType::default(),
            kind: "https://raise.io/ontology/arcadia/la#LogicalComponent".to_string(),
            properties: UnorderedMap::new(),
        };
        let action = WorldAction {
            intent: CommandType::Create,
        };
        assert!(engine.simulate(&element, action).is_ok());
        Ok(())
    }

    #[async_test]
    #[serial_test::serial] // Sécurité : L'orchestrateur charge l'IA
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_persistence_async() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await?;
        let config_app = AppConfig::get();
        let manager = CollectionsManager::new(
            &sandbox.db,
            &config_app.mount_points.system.domain,
            &config_app.mount_points.system.db,
        );

        let varmap = NeuralWeightsMap::new();
        let config = get_test_config();

        let engine1 = NeuroSymbolicEngine::new(config.clone(), varmap)?;
        engine1.save(&manager).await?;

        assert!(NeuroSymbolicEngine::exists(&manager).await);

        let engine2 = NeuroSymbolicEngine::load(&manager, config).await?;
        let element = ArcadiaElement {
            id: "t".to_string(),
            name: NameType::default(),
            kind: "test".to_string(),
            properties: UnorderedMap::new(),
        };
        let action = WorldAction {
            intent: CommandType::Search,
        };
        assert!(engine2.simulate(&element, action).is_ok());
        Ok(())
    }

    /// 🎯 NOUVEAU TEST : Résilience face à une partition système manquante
    #[async_test]
    #[serial_test::serial] // Sécurité : L'orchestrateur charge l'IA
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_resilience_missing_mount_point() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await?;
        // Manager pointant sur une partition fantôme
        let manager = CollectionsManager::new(&sandbox.db, "void_domain", "void_db");

        // Le moteur ne doit pas paniquer si le fichier n'existe pas
        assert!(!NeuroSymbolicEngine::exists(&manager).await);

        let config = get_test_config();
        let result = NeuroSymbolicEngine::load(&manager, config).await;

        match result {
            Err(AppError::Structured(err)) => {
                assert_eq!(err.code, "ERR_MODEL_NOT_FOUND");
                Ok(())
            }
            _ => panic!("Le moteur aurait dû lever ERR_MODEL_NOT_FOUND"),
        }
    }
}
