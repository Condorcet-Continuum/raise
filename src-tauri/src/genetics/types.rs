use crate::utils::{prelude::*, Ordering};
/// Structure représentant la performance d'un individu.
/// Conçue pour l'optimisation multi-objectifs (NSGA-II).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Fitness {
    /// Les valeurs des objectifs (ex: [latence, -coût]).
    /// Convention : On cherche toujours à MAXIMISER ces valeurs.
    /// (Pour minimiser un coût, il suffit de renvoyer une valeur négative ou d'inverser dans l'évaluateur).
    pub values: Vec<f32>,

    /// Score de violation de contrainte (0.0 = valide, >0.0 = invalide).
    /// Permet de guider l'évolution vers des solutions valides.
    pub constraint_violation: f32,

    // --- Métadonnées NSGA-II (calculées post-évaluation) ---
    /// Rang de dominance (0 = Front de Pareto optimal, 1 = Front suivant, etc.).
    #[serde(skip)]
    pub rank: usize,

    /// Distance de crowding (pour maintenir la diversité sur le front de Pareto).
    #[serde(skip)]
    pub crowding_distance: f32,
}

impl Fitness {
    /// Crée une nouvelle fitness brute (avant calcul de rang).
    pub fn new(values: Vec<f32>, constraint_violation: f32) -> Self {
        Self {
            values,
            constraint_violation,
            rank: usize::MAX, // Non classé par défaut
            crowding_distance: 0.0,
        }
    }

    /// Retourne true si self domine other.
    /// A domine B si A est au moins aussi bon que B partout et strictement meilleur sur au moins un critère.
    pub fn dominates(&self, other: &Fitness) -> bool {
        // 1. Priorité absolue à la validité (Constraint Handling)
        if self.constraint_violation < other.constraint_violation {
            return true; // Self est plus valide (ou valide tout court)
        }
        if self.constraint_violation > other.constraint_violation {
            return false;
        }

        // 2. Si contraintes égales, on compare les objectifs (Pareto)
        let mut at_least_one_better = false;
        for (a, b) in self.values.iter().zip(other.values.iter()) {
            if a < b {
                return false; // Pire sur un critère -> ne peut pas dominer
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

/// Un individu dans la population : un génome + sa performance.
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

/// La population complète pour une génération donnée.
#[derive(Clone, Debug)]
pub struct Population<G> {
    pub individuals: Vec<Individual<G>>,
    pub generation: usize,
}

impl<G> Default for Population<G> {
    fn default() -> Self {
        Self::new()
    }
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

    /// Retourne les n meilleurs individus (Élitisme simple basé sur le rang).
    /// Suppose que les rangs ont été calculés.
    pub fn get_elites(&self, count: usize) -> Vec<Individual<G>>
    where
        G: Clone,
    {
        let mut sorted_indices: Vec<usize> = (0..self.individuals.len()).collect();

        // Tri : Petit rang d'abord (meilleur), puis Grande distance (plus diversifié)
        sorted_indices.sort_by(|&a, &b| {
            let fit_a = self.individuals[a].fitness.as_ref().unwrap();
            let fit_b = self.individuals[b].fitness.as_ref().unwrap();

            match fit_a.rank.cmp(&fit_b.rank) {
                Ordering::Equal => fit_b
                    .crowding_distance
                    .partial_cmp(&fit_a.crowding_distance)
                    .unwrap_or(Ordering::Equal),
                other => other,
            }
        });

        sorted_indices
            .iter()
            .take(count)
            .map(|&idx| self.individuals[idx].clone())
            .collect()
    }
}

// --- Tests Unitaires ---
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fitness_dominance_logic() {
        // Cas 1 : Objectifs purs (Maximisation)
        let sol_a = Fitness::new(vec![10.0, 10.0], 0.0);
        let sol_b = Fitness::new(vec![5.0, 5.0], 0.0);
        let sol_pareto = Fitness::new(vec![12.0, 2.0], 0.0); // Meilleur obj1, pire obj2

        assert!(sol_a.dominates(&sol_b), "A(10,10) doit dominer B(5,5)");
        assert!(!sol_b.dominates(&sol_a), "B ne doit pas dominer A");

        // A et Pareto sont non-dominés l'un par l'autre
        assert!(
            !sol_a.dominates(&sol_pareto),
            "A ne domine pas Pareto (12 > 10)"
        );
        assert!(
            !sol_pareto.dominates(&sol_a),
            "Pareto ne domine pas A (10 > 2)"
        );
    }

    #[test]
    fn test_constraint_handling() {
        let valid = Fitness::new(vec![10.0], 0.0);
        let invalid_slight = Fitness::new(vec![100.0], 1.0); // Score élevé mais invalide
        let invalid_severe = Fitness::new(vec![100.0], 10.0);

        assert!(
            valid.dominates(&invalid_slight),
            "Valide domine toujours invalide"
        );
        assert!(
            invalid_slight.dominates(&invalid_severe),
            "Moins invalide domine plus invalide"
        );
    }
}
