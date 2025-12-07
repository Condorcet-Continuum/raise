use anyhow::Result;
use serde::{Deserialize, Serialize};

/// Le trait Genome définit la structure manipulable par l'AG.
/// Il doit être sérialisable pour être stocké dans la json_db.
pub trait Genome: Clone + Send + Sync + Serialize + for<'de> Deserialize<'de> {
    /// Génère un individu aléatoire (initialisation)
    fn random() -> Self;

    /// Applique une mutation sur le génome (modification in-place)
    fn mutate(&mut self, rate: f32);

    /// Croise deux génomes pour en produire un nouveau
    fn crossover(&self, other: &Self) -> Self;
}

/// Le trait Evaluator fait le lien avec le métier (GenAptitude Model Engine).
pub trait Evaluator<G: Genome>: Send + Sync {
    /// Calcule le score (fitness). Plus c'est haut, mieux c'est.
    /// Peut être asynchrone ou coûteux (ex: simulation).
    fn evaluate(&self, genome: &G) -> f32;

    /// Vérifie les "Hard Constraints" (ex: une fonction doit avoir un parent)
    fn is_valid(&self, genome: &G) -> bool {
        true
    }
}
