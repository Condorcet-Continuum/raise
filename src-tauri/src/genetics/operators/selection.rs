use crate::genetics::traits::Genome;
use crate::genetics::types::{Individual, Population};
use rand::prelude::*;

pub trait SelectionStrategy<G: Genome>: Send + Sync {
    fn select<'a>(&self, rng: &mut dyn RngCore, population: &'a Population<G>)
        -> &'a Individual<G>;
}

pub struct TournamentSelection {
    pub tournament_size: usize,
}

impl TournamentSelection {
    pub fn new(size: usize) -> Self {
        Self {
            tournament_size: size,
        }
    }
}

impl<G: Genome> SelectionStrategy<G> for TournamentSelection {
    fn select<'a>(
        &self,
        rng: &mut dyn RngCore,
        population: &'a Population<G>,
    ) -> &'a Individual<G> {
        let pop_len = population.individuals.len();
        if pop_len == 0 {
            panic!("Cannot select from empty population");
        }

        let mut best_candidate = &population.individuals[rng.random_range(0..pop_len)];

        for _ in 1..self.tournament_size {
            let challenger = &population.individuals[rng.random_range(0..pop_len)];

            if let (Some(fit_best), Some(fit_chal)) = (&best_candidate.fitness, &challenger.fitness)
            {
                // CORRECTION CLIPPY : Fusion des conditions identiques pour NSGA-II.
                // Le challenger devient le meilleur si :
                // 1. Son rang est meilleur (plus petit)
                // 2. OU son rang est égal mais sa distance de crowding est meilleure (plus grande).
                if fit_chal.rank < fit_best.rank
                    || (fit_chal.rank == fit_best.rank
                        && fit_chal.crowding_distance > fit_best.crowding_distance)
                {
                    best_candidate = challenger;
                }
            } else if challenger.fitness.is_some() && best_candidate.fitness.is_none() {
                best_candidate = challenger;
            }
        }
        best_candidate
    }
}

// --- Tests Unitaires ---
#[cfg(test)]
mod tests {
    use super::*;
    use crate::genetics::types::Fitness;

    #[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
    struct MockGenome;
    impl Genome for MockGenome {
        fn random() -> Self {
            MockGenome
        }
        fn mutate(&mut self, _: f32) {}
        fn crossover(&self, _: &Self) -> Self {
            MockGenome
        }
        fn distance(&self, _: &Self) -> f32 {
            0.0
        }
    }

    #[test]
    fn test_tournament_selection_logic() {
        let mut rng = rand::rng();
        let strategy = TournamentSelection::new(2);
        let mut pop = Population::new();

        // Individu A : Rang 1 (Meilleur)
        let mut ind_a = Individual::new(MockGenome);
        ind_a.fitness = Some(Fitness {
            rank: 1,
            crowding_distance: 10.0,
            ..Default::default()
        });

        // Individu B : Rang 2 (Moins bon)
        let mut ind_b = Individual::new(MockGenome);
        ind_b.fitness = Some(Fitness {
            rank: 2,
            crowding_distance: 100.0,
            ..Default::default()
        });

        pop.add(ind_a);
        pop.add(ind_b);

        // Sur 100 sélections, l'individu avec le meilleur rang (1) doit gagner
        let mut a_wins = 0;
        for _ in 0..100 {
            let selected = strategy.select(&mut rng, &pop);
            if selected.fitness.as_ref().unwrap().rank == 1 {
                a_wins += 1;
            }
        }

        // Statistiquement, avec un tournoi de 2, le meilleur doit être choisi très majoritairement
        assert!(a_wins > 60);
    }

    #[test]
    fn test_crowding_distance_tie_break() {
        let mut rng = rand::rng();
        let strategy = TournamentSelection::new(2);
        let mut pop = Population::new();

        // Même rang, mais B a une meilleure distance de crowding
        let mut ind_a = Individual::new(MockGenome);
        ind_a.fitness = Some(Fitness {
            rank: 1,
            crowding_distance: 10.0,
            ..Default::default()
        });

        let mut ind_b = Individual::new(MockGenome);
        ind_b.fitness = Some(Fitness {
            rank: 1,
            crowding_distance: 50.0,
            ..Default::default()
        });

        pop.add(ind_a);
        pop.add(ind_b);

        let mut b_wins = 0;
        for _ in 0..100 {
            let selected = strategy.select(&mut rng, &pop);
            if selected.fitness.as_ref().unwrap().crowding_distance > 40.0 {
                b_wins += 1;
            }
        }
        assert!(b_wins > 60);
    }
}
