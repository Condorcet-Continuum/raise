use candle_core::{Result, Tensor};
use candle_nn::{Activation, Init, Module, VarBuilder};

/// Cellule LSTM (Long Short-Term Memory) standard.
pub struct LSTMCell {
    pub weight_ih: Tensor, // Poids Input -> Hidden
    pub weight_hh: Tensor, // Poids Hidden -> Hidden
    pub bias_ih: Tensor,   // Biais Input
    pub bias_hh: Tensor,   // Biais Hidden
    pub hidden_size: usize,
}

impl LSTMCell {
    /// Initialise une nouvelle cellule LSTM avec des poids aléatoires.
    pub fn new(input_size: usize, hidden_size: usize, vb: VarBuilder) -> Result<Self> {
        let gate_size = 4 * hidden_size;

        // CORRECTION : Initialisation aléatoire (Randn) au lieu de Zéro
        // Cela permet au gradient de circuler dès le début.
        let weight_ih = vb.get_with_hints(
            (gate_size, input_size),
            "weight_ih",
            Init::Randn {
                mean: 0.,
                stdev: 0.1,
            },
        )?;

        let weight_hh = vb.get_with_hints(
            (gate_size, hidden_size),
            "weight_hh",
            Init::Randn {
                mean: 0.,
                stdev: 0.1,
            },
        )?;

        // Les biais peuvent rester à 0 au départ
        let bias_ih = vb.get_with_hints((gate_size,), "bias_ih", Init::Const(0.))?;
        let bias_hh = vb.get_with_hints((gate_size,), "bias_hh", Init::Const(0.))?;

        Ok(Self {
            weight_ih,
            weight_hh,
            bias_ih,
            bias_hh,
            hidden_size,
        })
    }

    pub fn forward(
        &self,
        input: &Tensor,
        hidden_state: &Tensor,
        cell_state: &Tensor,
    ) -> Result<(Tensor, Tensor)> {
        let w_ih_t = self.weight_ih.t()?;
        let w_hh_t = self.weight_hh.t()?;

        let inp_gates = input.matmul(&w_ih_t)?.broadcast_add(&self.bias_ih)?;
        let hid_gates = hidden_state.matmul(&w_hh_t)?.broadcast_add(&self.bias_hh)?;

        let gates = (inp_gates + hid_gates)?;

        let chunks = gates.chunk(4, 1)?;

        // Activations via le module standard
        let in_gate = Activation::Sigmoid.forward(&chunks[0])?;
        let forget_gate = Activation::Sigmoid.forward(&chunks[1])?;
        let cell_candidate = chunks[2].tanh()?;
        let out_gate = Activation::Sigmoid.forward(&chunks[3])?;

        let forget_term = forget_gate.mul(cell_state)?;
        let input_term = in_gate.mul(&cell_candidate)?;
        let next_cell_state = forget_term.add(&input_term)?;

        let next_hidden_state = out_gate.mul(&next_cell_state.tanh()?)?;

        Ok((next_hidden_state, next_cell_state))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use candle_core::{DType, Device};
    use candle_nn::VarMap;

    #[test]
    fn test_lstm_dimensions() -> Result<()> {
        let device = Device::Cpu;
        let varmap = VarMap::new();
        let vb = VarBuilder::from_varmap(&varmap, DType::F32, &device);

        let input_dim = 10;
        let hidden_dim = 20;
        let batch_size = 5;

        let lstm = LSTMCell::new(input_dim, hidden_dim, vb)?;

        let input = Tensor::randn(0f32, 1.0, (batch_size, input_dim), &device)?;
        let h0 = Tensor::zeros((batch_size, hidden_dim), DType::F32, &device)?;
        let c0 = Tensor::zeros((batch_size, hidden_dim), DType::F32, &device)?;

        let (h1, c1) = lstm.forward(&input, &h0, &c0)?;

        assert_eq!(h1.dims(), &[batch_size, hidden_dim]);
        assert_eq!(c1.dims(), &[batch_size, hidden_dim]);

        Ok(())
    }
}
