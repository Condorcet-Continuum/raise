// FICHIER : src-tauri/src/ai/world_model/engine.rs

use candle_core::{DType, Device, Tensor, Var};
use candle_nn::{VarBuilder, VarMap};

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
    pub fn to_tensor(&self, dim: usize) -> Result<Tensor> {
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

    #[allow(dead_code)]
    pub config_vocab_size: usize,
    #[allow(dead_code)]
    pub config_embedding_dim: usize,
    #[allow(dead_code)]
    pub config_action_dim: usize,
    #[allow(dead_code)]
    pub config_hidden_dim: usize,
}

impl NeuroSymbolicEngine {
    pub fn new(
        vocab_size: usize,
        embedding_dim: usize,
        action_dim: usize,
        hidden_dim: usize,
        varmap: VarMap,
    ) -> Result<Self> {
        let vb = VarBuilder::from_varmap(&varmap, DType::F32, &Device::Cpu);

        let quantizer = VectorQuantizer::new(vocab_size, embedding_dim, vb.pp("quantizer"))?;
        let predictor =
            WorldModelPredictor::new(embedding_dim, action_dim, hidden_dim, vb.pp("dynamics"))?;

        Ok(Self {
            varmap,
            quantizer,
            predictor,
            config_vocab_size: vocab_size,
            config_embedding_dim: embedding_dim,
            config_action_dim: action_dim,
            config_hidden_dim: hidden_dim,
        })
    }

    pub fn simulate(&self, element: &ArcadiaElement, action: WorldAction) -> Result<Tensor> {
        let raw_perception = ArcadiaEncoder::encode_element(element)?;
        let token = self.quantizer.tokenize(&raw_perception)?;
        let state_quantized = self.quantizer.decode(&token)?;
        let action_tensor = action.to_tensor(self.config_action_dim)?;

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

    pub async fn save_to_file<P: AsRef<Path>>(&self, path: P) -> Result<()> {
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
        vocab_size: usize,
        embedding_dim: usize,
        action_dim: usize,
        hidden_dim: usize,
    ) -> Result<Self> {
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

        Self::new(vocab_size, embedding_dim, action_dim, hidden_dim, varmap)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model_engine::types::NameType;
    use candle_nn::VarMap;
    use tempfile::NamedTempFile;

    #[test]
    fn test_engine_simulation_flow() {
        let varmap = VarMap::new();
        // CORRECTION : embedding_dim = 16 (8 layers + 8 categories) au lieu de 15
        let engine = NeuroSymbolicEngine::new(10, 16, 5, 32, varmap).unwrap();
        let element = ArcadiaElement {
            id: "1".to_string(),
            name: NameType::default(),
            kind: "https://raise.io/ontology/arcadia/la#LogicalComponent".to_string(), // URI valide
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
        // CORRECTION : embedding_dim = 16
        let engine1 = NeuroSymbolicEngine::new(10, 16, 5, 32, varmap).unwrap();
        engine1.save_to_file(path).await.expect("Save failed");

        // CORRECTION : embedding_dim = 16
        let engine2 = NeuroSymbolicEngine::load_from_file(path, 10, 16, 5, 32)
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
