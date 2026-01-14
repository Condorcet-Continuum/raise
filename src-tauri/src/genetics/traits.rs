// src-tauri/src/genetics/traits.rs

use serde::{Deserialize, Serialize};
use std::fmt::Debug;
use super::types::Fitness;

/// Le trait Genome définit la structure manipulable par l'AG.
pub trait Genome: Clone + Send + Sync + Debug + Serialize + for<'de> Deserialize<'de> {
    /// Génère un individu aléatoire (initialisation)
    fn random() -> Self;

    /// Applique une mutation sur le génome (modification in-place)
    fn mutate(&mut self, rate: f32);

    /// Croise deux génomes pour en produire un nouveau
    fn crossover(&self, other: &Self) -> Self;
    
    /// (Optionnel) Distance génétique entre deux génomes (pour la diversité)
    fn distance(&self, _other: &Self) -> f32 {
        0.0
    }
}

/// Le trait Evaluator fait le lien avec le métier.
/// Il retourne désormais un vecteur d'objectifs.
pub trait Evaluator<G: Genome>: Send + Sync {
    /// Nom des objectifs (pour l'affichage/debug)
    /// Ex: ["Performance", "-Coût"]
    fn objective_names(&self) -> Vec<String>;

    /// Calcule les scores. 
    /// Retourne (valeurs_objectifs, violation_contraintes).
    fn evaluate(&self, genome: &G) -> (Vec<f32>, f32);

    /// Vérification rapide de validité (Hard Constraints structurelles).
    /// Si false, on peut assigner une pénalité max sans calculer evaluate().
    fn is_valid(&self, _genome: &G) -> bool {
        true
    }
}

// --- Tests Unitaires ---
#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Debug, Serialize, Deserialize)]
    struct MockGenome(f32);

    impl Genome for MockGenome {
        fn random() -> Self { MockGenome(0.0) }
        fn mutate(&mut self, _rate: f32) { self.0 += 1.0; }
        fn crossover(&self, other: &Self) -> Self { MockGenome((self.0 + other.0)/2.0) }
    }

    struct MockEvaluator;
    impl Evaluator<MockGenome> for MockEvaluator {
        fn objective_names(&self) -> Vec<String> { vec!["Obj1".into()] }
        fn evaluate(&self, genome: &MockGenome) -> (Vec<f32>, f32) {
            (vec![genome.0], 0.0)
        }
    }

    #[test]
    fn test_traits_integration() {
        let mut g = MockGenome::random();
        g.mutate(0.1);
        let eval = MockEvaluator;
        let (scores, _) = eval.evaluate(&g);
        assert_eq!(scores[0], 1.0);
    }
}