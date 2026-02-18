use crate::genetics::operators::{crossover, mutation};
use crate::genetics::traits::Genome;
use crate::utils::prelude::*;
use rand::prelude::*;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NeuralNetworkGenome {
    pub weights: Vec<f32>,
    pub layer_sizes: Vec<usize>,
}

impl NeuralNetworkGenome {
    pub fn new_random(layer_sizes: Vec<usize>) -> Self {
        let total_weights = Self::calculate_total_weights(&layer_sizes);
        let mut rng = rand::rng(); // UPDATE

        let weights: Vec<f32> = (0..total_weights)
            .map(|_| rng.random_range(-1.0..1.0)) // UPDATE
            .collect();

        Self {
            weights,
            layer_sizes,
        }
    }

    fn calculate_total_weights(sizes: &[usize]) -> usize {
        let mut count = 0;
        for i in 0..sizes.len() - 1 {
            let n_in = sizes[i];
            let n_out = sizes[i + 1];
            count += (n_in * n_out) + n_out;
        }
        count
    }

    pub fn predict(&self, inputs: &[f32]) -> Vec<f32> {
        if self.layer_sizes.is_empty() || inputs.len() != self.layer_sizes[0] {
            panic!("Invalid input size");
        }

        let mut current_activations = inputs.to_vec();
        let mut weight_idx = 0;

        for i in 0..self.layer_sizes.len() - 1 {
            let n_in = self.layer_sizes[i];
            let n_out = self.layer_sizes[i + 1];
            let mut next_activations = vec![0.0; n_out];

            // CORRECTION CLIPPY: Utilisation de iter_mut() pour next_activations
            for activation in next_activations.iter_mut().take(n_out) {
                let mut sum = 0.0;

                // CORRECTION CLIPPY: Utilisation de iter() pour current_activations
                for &input_val in current_activations.iter().take(n_in) {
                    sum += input_val * self.weights[weight_idx];
                    weight_idx += 1;
                }

                sum += self.weights[weight_idx];
                weight_idx += 1;
                *activation = sum.tanh();
            }
            current_activations = next_activations;
        }

        current_activations
    }
}

impl Genome for NeuralNetworkGenome {
    fn random() -> Self {
        Self::new_random(vec![2, 3, 1])
    }

    fn mutate(&mut self, rate: f32) {
        let mut rng = rand::rng(); // UPDATE
        mutation::gaussian_mutation(&mut self.weights, rate, 0.1, &mut rng);
    }

    fn crossover(&self, other: &Self) -> Self {
        let mut rng = rand::rng(); // UPDATE
        let new_weights = crossover::uniform_crossover(&self.weights, &other.weights, &mut rng);

        Self {
            weights: new_weights,
            layer_sizes: self.layer_sizes.clone(),
        }
    }

    fn distance(&self, other: &Self) -> f32 {
        self.weights
            .iter()
            .zip(other.weights.iter())
            .map(|(a, b)| (a - b).powi(2))
            .sum::<f32>()
            .sqrt()
    }
}

// --- Tests Unitaires ---
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_network_structure() {
        let layers = vec![2, 2, 1];
        // L1->L2: (2*2 weights) + 2 biases = 6
        // L2->L3: (2*1 weights) + 1 bias = 3
        // Total = 9
        let genome = NeuralNetworkGenome::new_random(layers);
        assert_eq!(genome.weights.len(), 9);
    }

    #[test]
    fn test_prediction_flow() {
        let layers = vec![2, 2]; // Identité simple possible
        let mut genome = NeuralNetworkGenome::new_random(layers);

        // On force des poids à 0 et biais à 0
        genome.weights = vec![0.0; genome.weights.len()];

        let input = vec![1.0, -1.0];
        let output = genome.predict(&input);

        // Tout x 0 + 0 = 0. tanh(0) = 0.
        assert_eq!(output, vec![0.0, 0.0]);
    }

    #[test]
    fn test_mutation_changes_weights() {
        let layers = vec![2, 1];
        let mut genome = NeuralNetworkGenome::new_random(layers);
        let original_weights = genome.weights.clone();

        genome.mutate(1.0); // Force mutation

        // Probabilité infinitésimale que les poids soient identiques (float)
        assert_ne!(genome.weights, original_weights);
        assert_eq!(genome.weights.len(), original_weights.len());
    }
}
