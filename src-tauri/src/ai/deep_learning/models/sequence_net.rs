// FICHIER : src-tauri/src/ai/deep_learning/models/sequence_net.rs
use crate::utils::prelude::*;

/// Modèle de séquence complet (RNN natif).
pub struct SequenceNet {
    pub lstm: NeuralLstmLayer,
    pub head: NeuralLinearLayer,
    pub hidden_size: usize,
}

impl SequenceNet {
    pub fn new(
        input_size: usize,
        hidden_size: usize,
        output_size: usize,
        vb: NeuralWeightsBuilder,
    ) -> RaiseResult<Self> {
        // 🎯 L'inférence Default::default() trouve la bonne configuration toute seule !
        let lstm_layer =
            match init_lstm_layer(input_size, hidden_size, Default::default(), vb.pp("lstm")) {
                Ok(l) => l,
                Err(e) => raise_error!("ERR_SEQNET_LSTM_INIT", error = e.to_string()), // Plus de return Err()
            };

        let head_layer = match init_linear_layer(hidden_size, output_size, vb.pp("head")) {
            Ok(l) => l,
            Err(e) => raise_error!("ERR_SEQNET_HEAD_INIT", error = e.to_string()), // Plus de return Err()
        };

        Ok(Self {
            lstm: lstm_layer,
            head: head_layer,
            hidden_size,
        })
    }

    pub fn forward(&self, input_seq: &NeuralTensor) -> RaiseResult<NeuralTensor> {
        let (batch_size, seq_len, _) = match input_seq.dims3() {
            Ok(d) => d,
            Err(e) => raise_error!("ERR_SEQNET_DIMS", error = e.to_string()),
        };

        // 🎯 Initialisation de l'état caché (h, c) 100% géré par LSTM sur le GPU
        let mut state = match self.lstm.zero_state(batch_size) {
            Ok(s) => s,
            Err(e) => raise_error!("ERR_SEQNET_STATE", error = e.to_string()),
        };

        let mut outputs = Vec::with_capacity(seq_len);

        for t in 0..seq_len {
            let step_input = match input_seq.narrow(1, t, 1) {
                Ok(t) => match t.squeeze(1) {
                    Ok(t) => t,
                    Err(e) => raise_error!("ERR_SEQNET_SQUEEZE", error = e.to_string()),
                },
                Err(e) => raise_error!("ERR_SEQNET_NARROW", error = e.to_string()),
            };

            // 🎯 Exécution optimisée via la primitive .step() de LSTM
            state = match self.lstm.step(&step_input, &state) {
                Ok(s) => s,
                Err(e) => raise_error!("ERR_SEQNET_STEP", error = e.to_string()),
            };

            // L'état LSTM expose .h() pour obtenir le tenseur de sortie !
            let projection = match self.head.forward(state.h()) {
                Ok(t) => t,
                Err(e) => raise_error!("ERR_SEQNET_PROJECTION", error = e.to_string()),
            };

            outputs.push(projection);
        }

        match NeuralTensor::stack(&outputs, 1) {
            Ok(t) => Ok(t),
            Err(e) => raise_error!("ERR_SEQNET_STACK", error = e.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    fn test_sequence_net_flow() -> RaiseResult<()> {
        let device = ComputeHardware::Cpu;
        let varmap = NeuralWeightsMap::new();
        let vb = NeuralWeightsBuilder::from_varmap(&varmap, ComputeType::F32, &device);

        let batch_size = 2;
        let seq_len = 5;
        let input_dim = 10;
        let hidden_dim = 20;
        let output_dim = 50;

        let model = SequenceNet::new(input_dim, hidden_dim, output_dim, vb)?;

        let input = NeuralTensor::randn(0f32, 1.0, (batch_size, seq_len, input_dim), &device)?;
        let output = model.forward(&input)?;

        assert_eq!(output.dims(), &[batch_size, seq_len, output_dim]);

        let sum_sq = output.sqr()?.sum_all()?.to_scalar::<f32>()?;
        println!("Sum squares output: {}", sum_sq);
        assert!(sum_sq > 0.0, "Le modèle a produit une sortie nulle !");

        Ok(())
    }
}
