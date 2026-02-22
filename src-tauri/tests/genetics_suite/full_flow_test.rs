// FICHIER : src-tauri/tests/genetics_suite/full_flow_test.rs

use crate::common::setup_test_env;
use raise::genetics::bridge::GeneticsAdapter;
use raise::genetics::engine::GeneticConfig;
use raise::genetics::evaluators::architecture::ArchitectureEvaluator;
use raise::genetics::genomes::arcadia_arch::SystemAllocationGenome;
use raise::genetics::operators::selection::TournamentSelection;
use raise::genetics::types::{Individual, Population};
use raise::genetics::GeneticEngine;
use raise::json_db::collections::manager::CollectionsManager;
use raise::model_engine::loader::ModelLoader;
use raise::utils::prelude::*; // ✅ Correction : retrait de 'io' inutilisé

#[tokio::test]
#[ignore]
async fn test_arcadia_to_genetics_pipeline() {
    // 1. Initialisation robuste (Config + Schémas système)
    let env = setup_test_env().await;

    // 2. Création d'un manager sur un workspace spécifique au test
    let manager = CollectionsManager::new(&env.storage, "test_workspace", "arcadia_db");

    manager
        .insert_raw(
            "la",
            &json!({
                "id": "lf_nav_01",
                "name": "Navigation",
                "type": "https://raise.local/schemas/v1/arcadia/la/logical-function.schema.json",
                "properties": { "complexity": 45.0 }
            }),
        )
        .await
        .unwrap();

    manager
        .insert_raw(
            "la",
            &json!({
                "id": "lc_cpu_01",
                "name": "CPU",
                "type": "https://raise.local/schemas/v1/arcadia/la/logical-component.schema.json",
                "properties": { "capacity": 100.0 }
            }),
        )
        .await
        .unwrap();

    let loader = ModelLoader::new_with_manager(manager);
    let project_model = loader.load_full_model().await.expect("Erreur chargement");

    // Extraction des IDs pour le génome
    let function_ids: Vec<String> = project_model
        .la
        .functions
        .iter()
        .map(|f| f.id.clone())
        .collect();
    let component_ids: Vec<String> = project_model
        .pa
        .components
        .iter()
        .map(|c| c.id.clone())
        .collect();

    // Configuration et exécution du moteur génétique
    let adapter = GeneticsAdapter::new(&project_model);
    let cost_model = adapter.build_cost_model(&project_model);
    let evaluator = ArchitectureEvaluator::new(cost_model);

    let config = GeneticConfig {
        population_size: 20,
        max_generations: 5,
        ..Default::default()
    };

    let selection = TournamentSelection::new(2);
    let engine = GeneticEngine::new(evaluator, selection, config.clone());

    let mut initial_population = Population::new();
    for _ in 0..config.population_size {
        let genome =
            SystemAllocationGenome::new_random(function_ids.clone(), component_ids.clone());
        initial_population.add(Individual::new(genome));
    }

    let final_population = engine.run(initial_population, |_| {});
    assert!(!final_population.individuals.is_empty());
}
