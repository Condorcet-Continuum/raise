use crate::genetics::traits::Genome;
use crate::model_engine::types::ArcadiaElement;
use serde::{Deserialize, Serialize};

/// Un génome qui représente l'allocation de Fonctions (SA) sur des Composants (SA).
#[derive(Clone, Serialize, Deserialize)]
pub struct SystemAllocationGenome {
    // Map: ID Fonction -> ID Composant
    pub allocations: Vec<(String, String)>,
    pub available_components: Vec<String>,
}

impl Genome for SystemAllocationGenome {
    fn random() -> Self {
        // Logique pour créer une allocation aléatoire valide
        // ...
    }

    fn mutate(&mut self, rate: f32) {
        // Exemple : Déplacer une fonction d'un composant A vers un composant B
        // ...
    }

    // ...
}
