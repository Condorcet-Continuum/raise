#[tauri::command]
pub async fn run_optimization(
    storage: State<'_, StorageEngine>,
    // ... params ...
) -> Result<String, String> {
    // ... exécution de l'AG ...

    // À la fin, on sauvegarde le meilleur individu (Champion)
    let champion = population.best();

    let run_report = json!({
        "type": "optimization_run",
        "algorithm": "genetic_v1",
        "best_fitness": champion.fitness,
        "genome": champion.genome, // Le génome est sérialisé ici
        "parameters": { "mutation_rate": 0.01 }
    });

    // Utilisation de votre manager existant pour l'insertion
    let mgr = CollectionsManager::new(&storage, "un2", "_system");
    mgr.insert_with_schema("optimizations", run_report)
        .map_err(|e| e.to_string())?;

    Ok("Optimisation terminée et sauvegardée".to_string())
}
