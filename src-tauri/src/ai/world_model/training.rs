// FICHIER : src-tauri/src/ai/world_model/training.rs

use crate::utils::prelude::*;
use candle_nn::{AdamW, Optimizer, ParamsAdamW};

use crate::ai::world_model::engine::{NeuroSymbolicEngine, WorldAction};
use crate::ai::world_model::perception::ArcadiaEncoder;
use crate::model_engine::types::ArcadiaElement;

/// Le Coach du World Model.
pub struct WorldTrainer<'a> {
    engine: &'a NeuroSymbolicEngine,
    opt: AdamW,
}

impl<'a> WorldTrainer<'a> {
    pub fn new(engine: &'a NeuroSymbolicEngine, lr: f64) -> Result<Self> {
        let vars = engine.varmap.all_vars();
        // ✅ Conversion de l'erreur Candle lors de la création de l'optimiseur
        let opt = AdamW::new(
            vars,
            ParamsAdamW {
                lr,
                ..Default::default()
            },
        )
        .map_err(|e| AppError::from(e.to_string()))?;
        Ok(Self { engine, opt })
    }

    pub fn train_step(
        &mut self,
        state_t: &ArcadiaElement,
        action: WorldAction,
        state_t1_actual: &ArcadiaElement,
    ) -> Result<f64> {
        let predicted_tensor = self.engine.simulate(state_t, action)?;

        let raw_t1 = ArcadiaEncoder::encode_element(state_t1_actual)?;
        let token_t1 = self.engine.quantizer.tokenize(&raw_t1)?;
        let target_tensor = self.engine.quantizer.decode(&token_t1)?;
        let target_tensor = target_tensor.detach();

        // ✅ Conversion systématique des erreurs d'opérations sur les tenseurs (sub, sqr, mean)
        let diff = predicted_tensor
            .sub(&target_tensor)
            .map_err(|e| AppError::from(e.to_string()))?;

        let loss = diff
            .sqr()
            .map_err(|e| AppError::from(e.to_string()))?
            .mean_all()
            .map_err(|e| AppError::from(e.to_string()))?;

        // ✅ Conversion de l'erreur de l'étape d'optimisation
        self.opt
            .backward_step(&loss)
            .map_err(|e| AppError::from(e.to_string()))?;

        // ✅ Conversion de l'erreur de conversion en scalaire
        let scalar_loss = loss
            .to_scalar::<f32>()
            .map_err(|e| AppError::from(e.to_string()))? as f64;

        Ok(scalar_loss)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::nlp::parser::CommandType;
    use crate::model_engine::types::NameType;
    use crate::utils::HashMap;
    use candle_nn::VarMap;

    fn make_dummy(id: &str, layer_idx: usize) -> ArcadiaElement {
        let kind = match layer_idx {
            // CORRECTION : Utilisation des URIs officielles
            0 => "https://raise.io/ontology/arcadia/oa#OperationalActivity",
            _ => "https://raise.io/ontology/arcadia/la#LogicalFunction",
        };

        ArcadiaElement {
            id: id.to_string(),
            name: NameType::default(),
            kind: kind.to_string(),
            description: None,
            properties: HashMap::new(),
        }
    }

    #[test]
    fn test_training_loop_convergence() {
        // 1. Setup
        let varmap = VarMap::new();
        // CORRECTION : embedding_dim = 16 (Aligné avec l'encodeur V2)
        let engine = NeuroSymbolicEngine::new(10, 16, 5, 32, varmap).unwrap();

        // 2. Trainer
        let mut trainer = WorldTrainer::new(&engine, 0.05).unwrap();

        // 3. Données fictives
        let state_t = make_dummy("obs_1", 0);
        let state_t1 = make_dummy("obs_2", 2);

        // 4. Boucle d'entraînement
        let mut initial_loss = 0.0;
        let mut final_loss = 0.0;

        for i in 0..50 {
            let action = WorldAction {
                intent: CommandType::Create,
            };
            let loss = trainer.train_step(&state_t, action, &state_t1).unwrap();

            if i == 0 {
                initial_loss = loss;
            }
            final_loss = loss;
        }

        println!("Initial Loss: {}, Final Loss: {}", initial_loss, final_loss);
        assert!(final_loss < initial_loss, "Le modèle n'a pas appris !");
    }
}
