// src-tauri/src/genetics/types.rs

use serde::{Deserialize, Serialize};
use std::cmp::Ordering;

/// Structure représentant le résultat d'une évaluation complexe.
/// Compatible NSGA-II (Multi-Objective).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Fitness {
    /// Les valeurs des objectifs (ex: [minimiser latence, minimiser coût]).
    /// Par convention ici : Plus c'est haut, mieux c'est (Maximisation).
    /// Si vous voulez minimiser, inversez le signe dans l'Evaluator.
    pub values: Vec<f32>,
    
    /// Score de violation de contrainte (0.0 = valide, >0.0 = invalide).
    /// Utilisé pour pénaliser les solutions infaisables sans les jeter.
    pub constraint_violation: f32,

    // --- Champs spécifiques NSGA-II (calculés après évaluation) ---
    
    /// Rang de dominance (0 = Front de Pareto optimal).
    #[serde(skip)]
    pub rank: usize,
    
    /// Distance de crowding (pour maintenir la diversité sur le front).
    #[serde(skip)]
    pub crowding_distance: f32,
}

impl Fitness {
    pub fn new(values: Vec<f32>, constraint_violation: f32) -> Self {
        Self {
            values,
            constraint_violation,
            rank: 0,
            crowding_distance: 0.0,
        }
    }

    /// Retourne true si self domine other.
    /// Une solution A domine B si elle est au moins aussi bonne partout
    /// et strictement meilleure sur au moins un critère.
    pub fn dominates(&self, other: &Fitness) -> bool {
        // Priorité aux contraintes : une solution valide domine toujours une invalide
        if self.constraint_violation < other.constraint_violation {
            return true;
        }
        if self.constraint_violation > other.constraint_violation {
            return false;
        }

        // Si contraintes égales (ou nulles), on compare les objectifs
        let mut at_least_one_better = false;
        for (a, b) in self.values.iter().zip(other.values.iter()) {
            if a < b {
                return false; // Pire sur un critère -> ne domine pas
            }
            if a > b {
                at_least_one_better = true;
            }
        }
        at_least_one_better
    }
}

impl Default for Fitness {
    fn default() -> Self {
        Self {
            values: vec![],
            constraint_violation: 0.0,
            rank: usize::MAX,
            crowding_distance: 0.0,
        }
    }
}

/// Un individu dans la population.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Individual<G> {
    pub genome: G,
    pub fitness: Option<Fitness>,
}

impl<G> Individual<G> {
    pub fn new(genome: G) -> Self {
        Self {
            genome,
            fitness: None,
        }
    }
}

/// La population complète.
#[derive(Clone, Debug)]
pub struct Population<G> {
    pub individuals: Vec<Individual<G>>,
    pub generation: usize,
}

impl<G> Population<G> {
    pub fn new() -> Self {
        Self {
            individuals: vec![],
            generation: 0,
        }
    }

    pub fn len(&self) -> usize {
        self.individuals.len()
    }

    pub fn is_empty(&self) -> bool {
        self.individuals.is_empty()
    }

    pub fn add(&mut self, individual: Individual<G>) {
        self.individuals.push(individual);
    }
}

// --- Tests Unitaires ---
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dominance_logic() {
        // Maximize objectives
        let a = Fitness::new(vec![10.0, 10.0], 0.0);
        let b = Fitness::new(vec![5.0, 5.0], 0.0);
        let c = Fitness::new(vec![10.0, 5.0], 0.0);
        let d = Fitness::new(vec![12.0, 2.0], 0.0);

        assert!(a.dominates(&b), "A (10,10) doit dominer B (5,5)");
        assert!(a.dominates(&c), "A (10,10) doit dominer C (10,5)");
        assert!(!b.dominates(&a), "B ne doit pas dominer A");
        
        // A (10,10) vs D (12, 2) -> Aucun ne domine l'autre (Pareto)
        assert!(!a.dominates(&d), "A ne domine pas D car D est meilleur sur obj 1");
        assert!(!d.dominates(&a), "D ne domine pas A car A est meilleur sur obj 2");
    }

    #[test]
    fn test_constraints_dominance() {
        let valid = Fitness::new(vec![1.0], 0.0);
        let invalid_small = Fitness::new(vec![100.0], 1.0); // Gros score mais invalide
        let invalid_big = Fitness::new(vec![100.0], 5.0);   // Encore plus invalide

        assert!(valid.dominates(&invalid_small), "Valide doit toujours dominer invalide");
        assert!(invalid_small.dominates(&invalid_big), "Moins invalide domine plus invalide");
    }
}