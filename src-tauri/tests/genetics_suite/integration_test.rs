// FICHIER : src-tauri/tests/genetics_suite/integration_test.rs

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
async fn test_genetics_integration_with_json_db() {
    // 1. Initialisation robuste
    let env = setup_test_env().await;

    // 2. Manager sur un espace de test dédié
    let manager = CollectionsManager::new(&env.storage, "testing", "arcadia");

    let lf_schema = "https://raise.local/schemas/v1/arcadia/la/logical-function.schema.json";
    let lc_schema = "https://raise.local/schemas/v1/arcadia/la/logical-component.schema.json";

    manager.insert_raw("la", &json!({
        "id": "f_ctrl", "name": "Control", "type": lf_schema, "properties": { "complexity": 50.0 }
    })).await.unwrap();

    manager
        .insert_raw(
            "la",
            &json!({
                "id": "c_cpu", "name": "CPU", "type": lc_schema, "properties": { "capacity": 100.0 }
            }),
        )
        .await
        .unwrap();

    let loader = ModelLoader::new_with_manager(manager);
    let model = loader.load_full_model().await.expect("Erreur chargement");

    let function_ids: Vec<String> = model.la.functions.iter().map(|f| f.id.clone()).collect();
    let component_ids: Vec<String> = model.pa.components.iter().map(|c| c.id.clone()).collect();

    let adapter = GeneticsAdapter::new(&model);
    let cost_model = adapter.build_cost_model(&model);
    let evaluator = ArchitectureEvaluator::new(cost_model);

    let config = GeneticConfig {
        population_size: 10,
        max_generations: 2,
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
