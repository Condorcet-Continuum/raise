// FICHIER : src-tauri/tests/genetics_suite/integration_test.rs

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

#[async_test]
#[serial_test::serial]
#[cfg_attr(not(feature = "cuda"), ignore)]
async fn test_genetics_integration_with_json_db() -> RaiseResult<()> {
    let env = setup_test_env(LlmMode::Disabled).await?;

    // 🎯 FIX : On utilise la vraie partition métier (MBSE) initialisée par la sandbox
    let la_mgr = CollectionsManager::new(&env.sandbox.db, "un2", "la");

    // Insertion dans les bonnes collections (au lieu d'une collection générique 'la')
    la_mgr.insert_raw("functions", &json_value!({
        "_id": "f1", "name": "Control", "type": "LogicalFunction", "properties": { "complexity": 50.0 }
    })).await.unwrap();

    la_mgr.insert_raw("components", &json_value!({
        "_id": "c1", "name": "CPU", "type": "LogicalComponent", "properties": { "capacity": 100.0 }
    })).await.unwrap();

    // 🎯 FIX : Le ModelLoader a besoin du manager système pour lire la table 'configs' !
    let sys_mgr = CollectionsManager::new(&env.sandbox.db, &env.space, &env.db);
    let loader = ModelLoader::new_with_manager(sys_mgr)?;
    let model = loader.load_full_model().await.expect("Erreur chargement");

    let function_ids: Vec<String> = model
        .get_collection("la", "functions")
        .iter()
        .map(|f| f.id.clone())
        .collect();
    let component_ids: Vec<String> = model
        .get_collection("la", "components")
        .iter()
        .map(|c| c.id.clone())
        .collect();

    let adapter = GeneticsAdapter::new(&model);
    let cost_model = adapter.build_cost_model(&model);

    let config = GeneticConfig {
        population_size: 10,
        max_generations: 2,
        ..Default::default()
    };
    let engine = GeneticEngine::new(
        ArchitectureEvaluator::new(cost_model),
        TournamentSelection::new(2),
        config,
    );

    let mut pop = Population::new();
    pop.add(Individual::new(SystemAllocationGenome::new_random(
        function_ids,
        component_ids,
    )));

    let final_pop = engine.run(pop, |_| {});
    assert!(!final_pop.individuals.is_empty());

    Ok(())
}
