use crate::utils::prelude::*;

use crate::ai::deep_learning::layers::{linear::Linear, rnn_cell::LSTMCell};
use candle_core::{DType, Tensor};
use candle_nn::{Init, VarBuilder};

/// Modèle de séquence complet (RNN).
pub struct SequenceNet {
    pub lstm: LSTMCell,
    pub head: Linear,
    pub hidden_size: usize,
}

impl SequenceNet {
    pub fn new(
        input_size: usize,
        hidden_size: usize,
        output_size: usize,
        vb: VarBuilder,
    ) -> RaiseResult<Self> {
        let lstm = LSTMCell::new(input_size, hidden_size, vb.pp("lstm"))?;

        // CORRECTION : Initialisation aléatoire pour la tête de lecture aussi
        let head = Linear::new(
            vb.pp("head").get_with_hints(
                (output_size, hidden_size),
                "weight",
                Init::Randn {
                    mean: 0.,
                    stdev: 0.1,
                },
            )?,
            Some(
                vb.pp("head")
                    .get_with_hints((output_size,), "bias", Init::Const(0.))?,
            ),
        );

        Ok(Self {
            lstm,
            head,
            hidden_size,
        })
    }

    pub fn forward(&self, input_seq: &Tensor) -> RaiseResult<Tensor> {
        let (batch_size, seq_len, _) = input_seq.dims3()?;
        let device = input_seq.device();

        let mut h_state = Tensor::zeros((batch_size, self.hidden_size), DType::F32, device)?;
        let mut c_state = Tensor::zeros((batch_size, self.hidden_size), DType::F32, device)?;

        let mut outputs = Vec::with_capacity(seq_len);

        for t in 0..seq_len {
            let input_step = input_seq.narrow(1, t, 1)?.squeeze(1)?;
            let (next_h, next_c) = self.lstm.forward(&input_step, &h_state, &c_state)?;

            h_state = next_h;
            c_state = next_c;

            let projection = self.head.forward(&h_state)?;
            outputs.push(projection);
        }

        let result = Tensor::stack(&outputs, 1)?;
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use candle_core::Device;
    use candle_nn::VarMap;

    #[test]
    fn test_sequence_net_flow() -> RaiseResult<()> {
        let device = Device::Cpu;
        let varmap = VarMap::new();
        let vb = VarBuilder::from_varmap(&varmap, DType::F32, &device);

        let batch_size = 2;
        let seq_len = 5;
        let input_dim = 10;
        let hidden_dim = 20;
        let output_dim = 50;

        let model = SequenceNet::new(input_dim, hidden_dim, output_dim, vb)?;

        // Entrée aléatoire non nulle
        let input = Tensor::randn(0f32, 1.0, (batch_size, seq_len, input_dim), &device)?;

        let output = model.forward(&input)?;

        assert_eq!(output.dims(), &[batch_size, seq_len, output_dim]);

        // Cette assertion devrait maintenant passer car les poids ne sont plus à 0
        let sum_sq = output.sqr()?.sum_all()?.to_scalar::<f32>()?;
        println!("Sum squares output: {}", sum_sq); // Debug
        assert!(sum_sq > 0.0, "Le modèle a produit une sortie nulle !");

        Ok(())
    }
}
