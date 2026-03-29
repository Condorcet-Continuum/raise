// FICHIER : src-tauri/src/ai/world_model/engine.rs

use candle_core::{DType, Device, Tensor, Var};
use candle_nn::{VarBuilder, VarMap};

use crate::json_db::collections::manager::CollectionsManager;
use crate::utils::prelude::*;

use crate::ai::nlp::parser::CommandType;
use crate::ai::world_model::dynamics::WorldModelPredictor;
use crate::ai::world_model::perception::ArcadiaEncoder;
use crate::ai::world_model::representation::VectorQuantizer;
use crate::model_engine::types::ArcadiaElement;

pub struct WorldAction {
    pub intent: CommandType,
}

impl WorldAction {
    pub fn to_tensor(&self, dim: usize) -> RaiseResult<Tensor> {
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
        let data_len = data.len();
        Ok(match Tensor::from_vec(data, (1, dim), &Device::Cpu) {
            Ok(t) => t,
            Err(e) => raise_error!(
                "ERR_TENSOR_FROM_VEC",
                error = e,
                context = json_value!({
                    "action": "create_tensor_from_vec",
                    "expected_shape": [1, dim],
                    "data_len": data_len,
                    "device": "Cpu"
                })
            ),
        })
    }
}

pub struct NeuroSymbolicEngine {
    pub varmap: VarMap,
    pub quantizer: VectorQuantizer,
    pub predictor: WorldModelPredictor,
    pub config: WorldModelConfig,
}

impl NeuroSymbolicEngine {
    pub fn new(config: WorldModelConfig, varmap: VarMap) -> RaiseResult<Self> {
        let vb = VarBuilder::from_varmap(&varmap, DType::F32, &Device::Cpu);

        let quantizer = VectorQuantizer::new(&config, vb.pp("quantizer"))?;
        let predictor = WorldModelPredictor::new(&config, vb.pp("dynamics"))?;

        Ok(Self {
            varmap,
            quantizer,
            predictor,
            config,
        })
    }

    pub fn new_empty(config: WorldModelConfig) -> RaiseResult<Self> {
        let varmap = candle_nn::VarMap::new();
        Self::new(config, varmap)
    }

    pub fn simulate(&self, element: &ArcadiaElement, action: WorldAction) -> RaiseResult<Tensor> {
        let raw_perception = ArcadiaEncoder::encode_element(element)?;
        let token = self.quantizer.tokenize(&raw_perception)?;
        let state_quantized = self.quantizer.decode(&token)?;
        let action_tensor = action.to_tensor(self.config.action_dim)?;

        let predicted_future_state = self.predictor.forward(&state_quantized, &action_tensor)?;
        Ok(predicted_future_state)
    }

    fn extract_tensors_sync(&self) -> UnorderedMap<String, Tensor> {
        let data_guard = self.varmap.data().lock().unwrap();
        let mut extracted = UnorderedMap::new();
        for (k, v) in data_guard.iter() {
            extracted.insert(k.clone(), v.as_tensor().clone());
        }
        extracted
    }

    /// 🎯 ALIGNEMENT STRICT : Le Cerveau vit dans `domaine/db/tensors/world_model/`
    fn get_model_dir(manager: &CollectionsManager<'_>) -> PathBuf {
        manager
            .storage
            .config
            .db_root(&manager.space, &manager.db)
            .join("tensors")
            .join("world_model")
    }

    pub async fn save(&self, manager: &CollectionsManager<'_>) -> RaiseResult<()> {
        let model_dir = Self::get_model_dir(manager);
        fs::ensure_dir_async(&model_dir).await?;

        let path = model_dir.join("world_model.safetensors");
        let tensors = self.extract_tensors_sync();

        let path_display = path.to_string_lossy().to_string();
        let tensor_count = tensors.len();

        let spawn_result =
            spawn_cpu_task(move || candle_core::safetensors::save(&tensors, path)).await;

        let save_result = match spawn_result {
            Ok(res) => res,
            Err(e) => raise_error!(
                "ERR_ASYNC_SPAWN_FAILURE",
                error = e,
                context = json_value!({
                    "action": "spawn_blocking_save",
                    "path": path_display,
                    "hint": "La tâche a paniqué ou a été annulée."
                })
            ),
        };

        match save_result {
            Ok(_) => (),
            Err(e) => raise_error!(
                "ERR_MODEL_SAVE_SAFETENSORS",
                error = e,
                context = json_value!({
                    "action": "write_safetensors_to_disk",
                    "path": path_display,
                    "tensor_count": tensor_count
                })
            ),
        };
        Ok(())
    }

    pub async fn load(
        manager: &CollectionsManager<'_>,
        config: WorldModelConfig,
    ) -> RaiseResult<Self> {
        let model_dir = Self::get_model_dir(manager);
        let path = model_dir.join("world_model.safetensors");

        if !fs::exists_async(&path).await {
            raise_error!(
                "ERR_MODEL_NOT_FOUND",
                error = "Le fichier du World Model n'existe pas.",
                context = json_value!({ "path": path.to_string_lossy() })
            );
        }

        let buffer = fs::read_async(path).await?;

        let tensors = match candle_core::safetensors::load_buffer(&buffer, &Device::Cpu) {
            Ok(t) => t,
            Err(e) => raise_error!(
                "ERR_MODEL_LOAD_BUFFER",
                error = e,
                context = json_value!({
                    "action": "load_safetensors_buffer",
                    "buffer_size": buffer.len(),
                    "device": "Cpu",
                    "hint": "Le buffer est peut-être corrompu ou n'est pas au format Safetensors valide."
                })
            ),
        };

        let varmap = VarMap::new();
        {
            let mut data = varmap.data().lock().unwrap();
            for (name, tensor) in tensors {
                let var = match Var::from_tensor(&tensor) {
                    Ok(v) => v,
                    Err(e) => raise_error!(
                        "ERR_MODEL_VAR_CONVERSION",
                        error = e,
                        context = json_value!({
                            "action": "convert_tensor_to_var",
                            "tensor_name": name,
                            "shape": format!("{:?}", tensor.shape()),
                            "device": format!("{:?}", tensor.device())
                        })
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model_engine::types::NameType;
    use crate::utils::testing::AgentDbSandbox;
    use candle_nn::VarMap;

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
    fn test_engine_simulation_flow() {
        let varmap = VarMap::new();
        let config = get_test_config();

        let engine = NeuroSymbolicEngine::new(config, varmap).unwrap();

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
    }

    #[async_test]
    async fn test_persistence_async() {
        let sandbox = AgentDbSandbox::new().await;
        let manager = CollectionsManager::new(
            &sandbox.db,
            &sandbox.config.system_domain,
            &sandbox.config.system_db,
        );

        let varmap = VarMap::new();
        let config = get_test_config();

        let engine1 = NeuroSymbolicEngine::new(config.clone(), varmap).unwrap();
        // 🎯 Sauvegarde alignée sur la DB
        engine1.save(&manager).await.expect("Save failed");

        assert!(NeuroSymbolicEngine::exists(&manager).await);

        // 🎯 Chargement aligné sur la DB
        let engine2 = NeuroSymbolicEngine::load(&manager, config)
            .await
            .expect("Load failed");

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
    }
}
