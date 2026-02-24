// FICHIER : src-tauri/src/ai/world_model/engine.rs

use candle_core::{DType, Device, Tensor, Var};
use candle_nn::{VarBuilder, VarMap};

use crate::utils::config::WorldModelConfig;
use crate::utils::{io::Path, prelude::*, HashMap};

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

        Tensor::from_vec(data, (1, dim), &Device::Cpu).map_err(|e| AppError::from(e.to_string()))
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

        // L'encodeur n'est pas encore refactoré, on lui passe les champs manuellement
        let quantizer =
            VectorQuantizer::new(config.vocab_size, config.embedding_dim, vb.pp("quantizer"))?;

        // 🎯 Notre prédicteur prend enfin la config !
        let predictor = WorldModelPredictor::new(&config, vb.pp("dynamics"))?;

        Ok(Self {
            varmap,
            quantizer,
            predictor,
            config,
        })
    }

    pub fn simulate(&self, element: &ArcadiaElement, action: WorldAction) -> RaiseResult<Tensor> {
        let raw_perception = ArcadiaEncoder::encode_element(element)?;
        let token = self.quantizer.tokenize(&raw_perception)?;
        let state_quantized = self.quantizer.decode(&token)?;
        let action_tensor = action.to_tensor(self.config.action_dim)?;

        let predicted_future_state = self.predictor.forward(&state_quantized, &action_tensor)?;
        Ok(predicted_future_state)
    }

    fn extract_tensors_sync(&self) -> HashMap<String, Tensor> {
        let data_guard = self.varmap.data().lock().unwrap();
        let mut extracted = HashMap::new();
        for (k, v) in data_guard.iter() {
            extracted.insert(k.clone(), v.as_tensor().clone());
        }
        extracted
    }

    pub async fn save_to_file<P: AsRef<Path>>(&self, path: P) -> RaiseResult<()> {
        let path = path.as_ref().to_owned();
        let tensors = self.extract_tensors_sync();

        tokio::task::spawn_blocking(move || candle_core::safetensors::save(&tensors, path))
            .await
            .map_err(|e| AppError::from(format!("Spawn error: {}", e)))?
            .map_err(|e| AppError::from(format!("Save error: {}", e)))?;
        Ok(())
    }

    pub async fn load_from_file<P: AsRef<Path>>(
        path: P,
        config: WorldModelConfig,
    ) -> RaiseResult<Self> {
        let buffer = tokio::fs::read(path).await?;

        let tensors = candle_core::safetensors::load_buffer(&buffer, &Device::Cpu)
            .map_err(|e| AppError::from(e.to_string()))?;

        let varmap = VarMap::new();
        {
            let mut data = varmap.data().lock().unwrap();
            for (name, tensor) in tensors {
                data.insert(
                    name,
                    Var::from_tensor(&tensor).map_err(|e| AppError::from(e.to_string()))?,
                );
            }
        }

        Self::new(config, varmap)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model_engine::types::NameType;
    use crate::utils::config::WorldModelConfig;
    use candle_nn::VarMap;
    use tempfile::NamedTempFile; // 🎯 NOUVEL IMPORT

    // Helper pour générer une config de test
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

        // 🎯 On passe la config au lieu des 4 entiers
        let engine = NeuroSymbolicEngine::new(config, varmap).unwrap();

        let element = ArcadiaElement {
            id: "1".to_string(),
            name: NameType::default(),
            kind: "https://raise.io/ontology/arcadia/la#LogicalComponent".to_string(),
            description: None,
            properties: HashMap::new(),
        };
        let action = WorldAction {
            intent: CommandType::Create,
        };
        assert!(engine.simulate(&element, action).is_ok());
    }

    #[tokio::test]
    async fn test_persistence_async() {
        let file = NamedTempFile::new().unwrap();
        let path = file.path();
        let varmap = VarMap::new();
        let config = get_test_config();

        let engine1 = NeuroSymbolicEngine::new(config.clone(), varmap).unwrap();
        engine1.save_to_file(path).await.expect("Save failed");

        // 🎯 load_from_file prend maintenant la config
        let engine2 = NeuroSymbolicEngine::load_from_file(path, config)
            .await
            .expect("Load failed");

        let element = ArcadiaElement {
            id: "t".to_string(),
            name: NameType::default(),
            kind: "test".to_string(),
            description: None,
            properties: HashMap::new(),
        };
        let action = WorldAction {
            intent: CommandType::Search,
        };
        assert!(engine2.simulate(&element, action).is_ok());
    }
}
