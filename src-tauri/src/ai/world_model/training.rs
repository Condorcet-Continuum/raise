// FICHIER : src-tauri/src/ai/world_model/training.rs

use anyhow::Result;
// CORRECTION : Suppression de 'use candle_core::Tensor;' qui était inutile
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
        // Accès aux vars (possible car engine.varmap est public)
        let vars = engine.varmap.all_vars();
        let opt = AdamW::new(
            vars,
            ParamsAdamW {
                lr,
                ..Default::default()
            },
        )?;
        Ok(Self { engine, opt })
    }

    pub fn train_step(
        &mut self,
        state_t: &ArcadiaElement,
        action: WorldAction,
        state_t1_actual: &ArcadiaElement,
    ) -> Result<f64> {
        // 1. Simulation (Prédiction)
        let predicted_tensor = self.engine.simulate(state_t, action)?;

        // 2. Cible (Ground Truth)
        let raw_t1 = ArcadiaEncoder::encode_element(state_t1_actual)?;
        let token_t1 = self.engine.quantizer.tokenize(&raw_t1)?;
        let target_tensor = self.engine.quantizer.decode(&token_t1)?;

        // On détache la cible du graphe de calcul pour ne pas backpropager dedans
        let target_tensor = target_tensor.detach();

        // 3. Loss (MSE)
        // Utilisation de .sub() pour la soustraction sûre
        let diff = predicted_tensor.sub(&target_tensor)?;
        let loss = diff.sqr()?.mean_all()?;

        // 4. Backprop (Apprentissage)
        self.opt.backward_step(&loss)?;

        let scalar_loss = loss.to_scalar::<f32>()? as f64;
        Ok(scalar_loss)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::nlp::parser::CommandType;
    use crate::model_engine::types::NameType;
    use candle_nn::VarMap;
    use std::collections::HashMap;

    fn make_dummy(id: &str, layer_idx: usize) -> ArcadiaElement {
        let kind = match layer_idx {
            0 => "https://arcadia/oa#OperationalActivity",
            _ => "https://arcadia/la#LogicalFunction",
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
        // On initialise le moteur
        let engine = NeuroSymbolicEngine::new(10, 15, 5, 32, varmap).unwrap();

        // 2. Trainer avec un fort taux d'apprentissage pour le test
        let mut trainer = WorldTrainer::new(&engine, 0.05).unwrap();

        // 3. Données fictives (Transition OA -> LA via Create)
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

        // 5. Vérification : L'erreur doit avoir diminué
        assert!(final_loss < initial_loss, "Le modèle n'a pas appris !");
    }
}
