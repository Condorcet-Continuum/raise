use crate::genetics::operators::{crossover, mutation};
use crate::genetics::traits::Genome;
use rand::prelude::*;
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Clone, Serialize, Deserialize)]
pub struct SystemAllocationGenome {
    pub genes: Vec<usize>,
    pub function_ids: Vec<String>,
    pub available_component_ids: Vec<String>,
}

impl SystemAllocationGenome {
    pub fn new_template(function_ids: Vec<String>, component_ids: Vec<String>) -> Self {
        Self {
            genes: vec![0; function_ids.len()],
            function_ids,
            available_component_ids: component_ids,
        }
    }

    pub fn get_allocations(&self) -> Vec<(String, String)> {
        self.function_ids
            .iter()
            .zip(self.genes.iter())
            .map(|(func_id, &comp_idx)| {
                (
                    func_id.clone(),
                    self.available_component_ids[comp_idx].clone(),
                )
            })
            .collect()
    }
}

impl Genome for SystemAllocationGenome {
    fn random() -> Self {
        panic!("Use SystemAllocationGenome::new_random(...) instead")
    }

    fn mutate(&mut self, rate: f32) {
        let num_components = self.available_component_ids.len();
        if num_components == 0 {
            return;
        }

        let mut rng = rand::rng(); // UPDATE

        // UPDATE: random_range
        mutation::uniform_mutation(&mut self.genes, rate, &mut rng, |r| {
            r.random_range(0..num_components)
        });
    }

    fn crossover(&self, other: &Self) -> Self {
        let mut rng = rand::rng(); // UPDATE
        let new_genes = crossover::uniform_crossover(&self.genes, &other.genes, &mut rng);

        Self {
            genes: new_genes,
            function_ids: self.function_ids.clone(),
            available_component_ids: self.available_component_ids.clone(),
        }
    }

    fn distance(&self, other: &Self) -> f32 {
        self.genes
            .iter()
            .zip(other.genes.iter())
            .filter(|(a, b)| a != b)
            .count() as f32
    }
}

impl SystemAllocationGenome {
    pub fn new_random(function_ids: Vec<String>, component_ids: Vec<String>) -> Self {
        let mut rng = rand::rng(); // UPDATE
        let len = function_ids.len();
        let comp_count = component_ids.len();

        // UPDATE: random_range
        let genes: Vec<usize> = (0..len).map(|_| rng.random_range(0..comp_count)).collect();

        Self {
            genes,
            function_ids,
            available_component_ids: component_ids,
        }
    }
}

impl fmt::Debug for SystemAllocationGenome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "AllocGenome(sz={}, genes={:?})",
            self.genes.len(),
            self.genes
        )
    }
}

// --- Tests Unitaires ---
#[cfg(test)]
mod tests {
    use super::*;

    fn mock_context() -> (Vec<String>, Vec<String>) {
        (
            vec!["F1".into(), "F2".into(), "F3".into()], // 3 Fonctions
            vec!["C1".into(), "C2".into()],              // 2 Composants
        )
    }

    #[test]
    fn test_initialization_and_mapping() {
        let (f_ids, c_ids) = mock_context();

        // Force genes to [0, 1, 0] -> F1->C1, F2->C2, F3->C1
        let genome = SystemAllocationGenome {
            genes: vec![0, 1, 0],
            function_ids: f_ids,
            available_component_ids: c_ids,
        };

        let mapping = genome.get_allocations();
        assert_eq!(mapping[0], ("F1".to_string(), "C1".to_string()));
        assert_eq!(mapping[1], ("F2".to_string(), "C2".to_string()));
        assert_eq!(mapping[2], ("F3".to_string(), "C1".to_string()));
    }

    #[test]
    fn test_mutation_changes_genes() {
        let (f_ids, c_ids) = mock_context();
        let mut genome = SystemAllocationGenome::new_random(f_ids, c_ids);
        let original_genes = genome.genes.clone();

        // Mutation rate 1.0 = Changement garanti (statistiquement presque sûr)
        genome.mutate(1.0);

        // Il y a une petite probabilité que le random retombe sur le même index,
        // mais sur un vecteur entier, ça doit changer.
        assert_ne!(genome.genes, original_genes, "Le génome doit muter");
        // La structure structurelle ne doit pas changer
        assert_eq!(genome.genes.len(), original_genes.len());
    }

    #[test]
    fn test_crossover_mixes_parents() {
        let (f_ids, c_ids) = mock_context();

        let parent1 = SystemAllocationGenome {
            genes: vec![0, 0, 0],
            function_ids: f_ids.clone(),
            available_component_ids: c_ids.clone(),
        };

        let parent2 = SystemAllocationGenome {
            genes: vec![1, 1, 1],
            function_ids: f_ids,
            available_component_ids: c_ids,
        };

        let child = parent1.crossover(&parent2);

        // L'enfant doit avoir la même taille
        assert_eq!(child.genes.len(), 3);

        // L'enfant doit contenir uniquement des 0 ou des 1
        for gene in child.genes {
            assert!(gene == 0 || gene == 1);
        }
    }
}
