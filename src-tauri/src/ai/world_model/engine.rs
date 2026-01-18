// FICHIER : src-tauri/src/ai/world_model/engine.rs

use anyhow::Result;
use candle_core::{DType, Device, Tensor, Var};
use candle_nn::{VarBuilder, VarMap};
use std::collections::HashMap;
use std::path::Path;

// Imports des types existants
use crate::ai::nlp::parser::CommandType;
use crate::model_engine::types::ArcadiaElement;

// Imports des sous-modules
use crate::ai::world_model::dynamics::WorldModelPredictor;
use crate::ai::world_model::perception::ArcadiaEncoder;
use crate::ai::world_model::representation::VectorQuantizer;

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

        Ok(Tensor::from_vec(data, (1, dim), &Device::Cpu)?)
    }
}

/// Moteur principal du World Model.
pub struct NeuroSymbolicEngine {
    pub varmap: VarMap,
    pub quantizer: VectorQuantizer,
    pub predictor: WorldModelPredictor,

    // Configurations
    #[allow(dead_code)]
    pub config_vocab_size: usize,
    #[allow(dead_code)]
    pub config_embedding_dim: usize,
    #[allow(dead_code)]
    pub config_action_dim: usize,
    #[allow(dead_code)]
    pub config_hidden_dim: usize,
}

// DÉBUT DU BLOC D'IMPLÉMENTATION
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

    // --- HELPER FUNCTION SYNCHRONE ---
    // Cette fonction EST BIEN À L'INTÉRIEUR du bloc impl NeuroSymbolicEngine
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

        // Appel de la méthode synchrone (le self la voit car elle est dans le même impl)
        let tensors = self.extract_tensors_sync();

        // Sauvegarde asynchrone (le Mutex est déjà relâché)
        tokio::task::spawn_blocking(move || candle_core::safetensors::save(&tensors, path))
            .await??;

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
        let tensors = candle_core::safetensors::load_buffer(&buffer, &Device::Cpu)?;

        let varmap = VarMap::new();
        {
            let mut data = varmap.data().lock().unwrap();
            for (name, tensor) in tensors {
                data.insert(name, Var::from_tensor(&tensor)?);
            }
        }

        Self::new(vocab_size, embedding_dim, action_dim, hidden_dim, varmap)
    }
} // FIN DU BLOC D'IMPLÉMENTATION

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model_engine::types::NameType;
    use candle_nn::VarMap;
    use std::collections::HashMap;
    use tempfile::NamedTempFile;

    #[test]
    fn test_engine_simulation_flow() {
        let varmap = VarMap::new();
        let engine = NeuroSymbolicEngine::new(10, 15, 5, 32, varmap).unwrap();
        let element = ArcadiaElement {
            id: "1".to_string(),
            name: NameType::default(),
            kind: "https://arcadia/la#LogicalComponent".to_string(),
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
        let engine1 = NeuroSymbolicEngine::new(10, 15, 5, 32, varmap).unwrap();
        engine1.save_to_file(path).await.expect("Save failed");

        let engine2 = NeuroSymbolicEngine::load_from_file(path, 10, 15, 5, 32)
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
