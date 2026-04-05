// FICHIER : src-tauri/tests/genetics_suite/full_flow_test.rs

use crate::common::{setup_test_env, LlmMode};
use raise::genetics::bridge::GeneticsAdapter;
use raise::genetics::engine::GeneticConfig;
use raise::genetics::evaluators::architecture::ArchitectureEvaluator;
use raise::genetics::genomes::arcadia_arch::SystemAllocationGenome;
use raise::genetics::operators::selection::TournamentSelection;
use raise::genetics::types::{Individual, Population};
use raise::genetics::GeneticEngine;
use raise::json_db::collections::manager::CollectionsManager;
use raise::model_engine::loader::ModelLoader;
use raise::utils::prelude::*;
use raise::utils::testing::DbSandbox;

#[async_test]
async fn test_arcadia_to_genetics_pipeline() {
    let env = setup_test_env(LlmMode::Disabled).await;
    let manager = CollectionsManager::new(&env.sandbox.storage, "test_workspace", "arcadia_db");
    DbSandbox::mock_db(&manager).await.unwrap();
    // Injections de test
    let _ = manager
        .create_collection("la", "db://_system/schemas/v1/db/generic.schema.json")
        .await;
    manager.insert_raw("la", &json_value!({ "_id": "F1", "name": "Nav", "type": "LogicalFunction", "properties": {"complexity": 45.0}})).await.unwrap();
    manager.insert_raw("la", &json_value!({ "_id": "C1", "name": "CPU", "type": "LogicalComponent", "properties": {"capacity": 100.0}})).await.unwrap();

    let loader = ModelLoader::new_with_manager(manager);
    let project_model = loader.load_full_model().await.expect("Erreur chargement");

    // 🎯 PURE GRAPH : Extraction via get_collection
    let function_ids: Vec<String> = project_model
        .get_collection("la", "functions")
        .iter()
        .map(|f| f.id.clone())
        .collect();
    let component_ids: Vec<String> = project_model
        .get_collection("la", "components")
        .iter()
        .map(|c| c.id.clone())
        .collect();

    let adapter = GeneticsAdapter::new(&project_model);
    let cost_model = adapter.build_cost_model(&project_model);
    let engine = GeneticEngine::new(
        ArchitectureEvaluator::new(cost_model),
        TournamentSelection::new(2),
        GeneticConfig::default(),
    );

    let mut pop = Population::new();
    pop.add(Individual::new(SystemAllocationGenome::new_random(
        function_ids,
        component_ids,
    )));

    let final_pop = engine.run(pop, |_| {});
    assert!(!final_pop.individuals.is_empty());
}
