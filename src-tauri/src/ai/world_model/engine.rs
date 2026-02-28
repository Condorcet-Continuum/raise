// FICHIER : src-tauri/src/ai/world_model/engine.rs

use candle_core::{DType, Device, Tensor, Var};
use candle_nn::{VarBuilder, VarMap};

use crate::utils::config::WorldModelConfig;
use crate::utils::{io, prelude::*, HashMap};

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
                context = json!({
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

        // 1. On extrait les infos pour les logs AVANT le move
        let path_display = path.to_string_lossy().to_string();
        let tensor_count = tensors.len();

        // 2. On attend la fin de la tâche (les originaux sont déplacés ici)
        let spawn_result =
            tokio::task::spawn_blocking(move || candle_core::safetensors::save(&tensors, path))
                .await;

        // 3. Gestion de l'erreur de Spawn
        let save_result = match spawn_result {
            Ok(res) => res,
            Err(e) => raise_error!(
                "ERR_ASYNC_SPAWN_FAILURE",
                error = e,
                context = json!({
                    "action": "spawn_blocking_save",
                    "path": path_display, // On utilise la copie légère
                    "hint": "La tâche a paniqué ou a été annulée."
                })
            ),
        };

        // 4. Gestion de l'erreur de sauvegarde
        match save_result {
            Ok(_) => (),
            Err(e) => raise_error!(
                "ERR_MODEL_SAVE_SAFETENSORS",
                error = e,
                context = json!({
                    "action": "write_safetensors_to_disk",
                    "path": path_display, // On utilise la copie légère
                    "tensor_count": tensor_count
                })
            ),
        };
        Ok(())
    }

    pub async fn load_from_file<P: AsRef<Path>>(
        path: P,
        config: WorldModelConfig,
    ) -> RaiseResult<Self> {
        let buffer = io::read(path).await?;

        let tensors = match candle_core::safetensors::load_buffer(&buffer, &Device::Cpu) {
            Ok(t) => t,
            Err(e) => raise_error!(
                "ERR_MODEL_LOAD_BUFFER",
                error = e,
                context = json!({
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
                        context = json!({
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
