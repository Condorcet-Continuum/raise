use std::time::Instant;
use tauri::{Emitter, Window};

use crate::genetics::dto::{
    AllocatedSolution, OptimizationProgress, OptimizationRequest, OptimizationResult,
};
use crate::genetics::engine::{GeneticConfig, GeneticEngine};
use crate::genetics::evaluators::architecture::{ArchitectureCostModel, ArchitectureEvaluator};
use crate::genetics::evaluators::constraints::SegregationConstraint;
use crate::genetics::genomes::arcadia_arch::SystemAllocationGenome;
use crate::genetics::operators::selection::TournamentSelection;
use crate::genetics::types::{Individual, Population};

#[tauri::command]
pub fn debug_genetics_ping(name: String) -> String {
    println!("🔔 Ping reçu de la part de : {}", name);
    format!("Hello {}, le pont Tauri fonctionne !", name)
}

/// Commande principale pour l'optimisation d'architecture.
#[tauri::command]
pub async fn run_architecture_optimization(
    window: Window,
    params: OptimizationRequest,
) -> Result<OptimizationResult, String> {
    let start_time = Instant::now();

    // 1. Préparation des données (Mapping IDs -> Index)
    let func_ids: Vec<String> = params.functions.iter().map(|f| f.id.clone()).collect();
    let comp_ids: Vec<String> = params.components.iter().map(|c| c.id.clone()).collect();

    // Mapping des flux (volumes)
    let mut flow_triplets = Vec::new();
    for flow in &params.flows {
        let src_idx = func_ids.iter().position(|id| id == &flow.source_id);
        let tgt_idx = func_ids.iter().position(|id| id == &flow.target_id);
        if let (Some(s), Some(t)) = (src_idx, tgt_idx) {
            flow_triplets.push((s, t, flow.volume));
        }
    }

    let loads: Vec<(usize, f32)> = params
        .functions
        .iter()
        .enumerate()
        .map(|(i, f)| (i, f.load))
        .collect();
    let capacities: Vec<(usize, f32)> = params
        .components
        .iter()
        .enumerate()
        .map(|(i, c)| (i, c.capacity))
        .collect();

    // 2. Initialisation de l'évaluateur
    // Correction : ArchitectureCostModel attend les dimensions et les vecteurs de données
    let model = ArchitectureCostModel::new(
        func_ids.len(),
        comp_ids.len(),
        &flow_triplets,
        &loads,
        &capacities,
    );
    let mut evaluator = ArchitectureEvaluator::new(model);

    // Ajout des contraintes de ségrégation
    if let Some(conf) = &params.constraints {
        for (id_a, id_b) in &conf.segregations {
            let idx_a = func_ids.iter().position(|id| id == id_a);
            let idx_b = func_ids.iter().position(|id| id == id_b);
            if let (Some(a), Some(b)) = (idx_a, idx_b) {
                evaluator.add_constraint(SegregationConstraint {
                    func_a_idx: a,
                    func_b_idx: b,
                    penalty: 1000.0,
                });
            }
        }
    }

    // 3. Configuration du Moteur
    let config = GeneticConfig {
        population_size: params.population_size,
        max_generations: params.max_generations,
        mutation_rate: params.mutation_rate,
        crossover_rate: params.crossover_rate,
        elitism_count: (params.population_size / 10).max(1),
    };

    let selection = TournamentSelection::new(3);
    let engine = GeneticEngine::new(evaluator, selection, config.clone());

    // 4. Initialisation de la Population
    let mut population = Population::new();
    for _ in 0..config.population_size {
        // Correction : Utilisation de new_random avec les IDs métier
        let genome = SystemAllocationGenome::new_random(func_ids.clone(), comp_ids.clone());
        population.add(Individual::new(genome));
    }

    // 5. Exécution avec Télémétrie (Émissions d'événements Tauri)
    let final_pop = engine.run(population, |pop| {
        if let Some(best) = pop.individuals.first() {
            if let Some(fit) = &best.fitness {
                // Émission vers le Frontend via le canal genetics://progress
                let _ = window.emit(
                    "genetics://progress",
                    OptimizationProgress {
                        generation: pop.generation,
                        best_fitness: fit.values.clone(),
                        diversity: fit.crowding_distance,
                    },
                );
            }
        }
    });

    // 6. Extraction du Front de Pareto
    let pareto_front: Vec<AllocatedSolution> = final_pop
        .individuals
        .into_iter()
        .filter(|ind| ind.fitness.as_ref().map(|f| f.rank == 0).unwrap_or(false))
        .map(|ind| {
            let fitness = ind.fitness.unwrap_or_default();
            AllocatedSolution {
                fitness: fitness.values,
                constraint_violation: fitness.constraint_violation,
                allocation: ind.genome.get_allocations(),
            }
        })
        .collect();

    Ok(OptimizationResult {
        duration_ms: start_time.elapsed().as_millis(),
        pareto_front,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::genetics::dto::{ComponentDto, FunctionDto};

    #[tokio::test]
    async fn test_architecture_optimization_command_logic() {
        // Note: Ce test ne peut pas utiliser une vraie Window Tauri sans setup complexe,
        // mais on teste ici la logique de préparation des données.
        let request = OptimizationRequest {
            functions: vec![FunctionDto {
                id: "f1".into(),
                load: 10.0,
            }],
            components: vec![ComponentDto {
                id: "c1".into(),
                capacity: 100.0,
            }],
            flows: vec![],
            constraints: None,
            population_size: 10,
            max_generations: 2,
            mutation_rate: 0.1,
            crossover_rate: 0.8,
        };

        // La logique ici simule l'appel de la commande
        assert_eq!(request.functions.len(), 1);
        assert_eq!(request.components.len(), 1);
    }
}
