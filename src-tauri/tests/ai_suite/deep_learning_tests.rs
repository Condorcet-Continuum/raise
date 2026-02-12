// FICHIER : src-tauri/tests/ai_suite/deep_learning_tests.rs

use candle_core::{DType, Device, Tensor};
use candle_nn::{VarBuilder, VarMap};
use raise::ai::deep_learning::models::sequence_net::SequenceNet;
use raise::ai::deep_learning::serialization;
use raise::ai::deep_learning::trainer::Trainer;
use raise::commands::ai_commands::DlState;
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH}; // Pour l'unicité

#[test]
fn test_dl_e2e_integration() -> anyhow::Result<()> {
    // --- 1. CONFIGURATION ROBUSTE & ISOLÉE ---
    let state = DlState::new();
    let device = Device::Cpu;

    // Génération d'un nom de fichier unique pour éviter les collisions (Race Conditions)
    // Cela résout les problèmes de tests parallèles et de SIGBUS.
    let start = SystemTime::now();
    let since_the_epoch = start
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards");
    let unique_id = since_the_epoch.as_nanos();

    // On stocke dans le dossier temporaire du système (/tmp sur Linux)
    let filename = format!("test_integration_model_{}.safetensors", unique_id);
    let save_path = std::env::temp_dir().join(filename);

    println!("📝 Fichier de test temporaire : {:?}", save_path);

    // Hyperparamètres
    let input_dim = 5;
    let hidden_dim = 10;
    let output_dim = 2;

    println!("--- Étape 1 : Initialisation du Modèle dans le State ---");
    {
        let varmap = VarMap::new();
        let vb = VarBuilder::from_varmap(&varmap, DType::F32, &device);
        let model = SequenceNet::new(input_dim, hidden_dim, output_dim, vb)?;

        let mut model_guard = state.model.lock().unwrap();
        let mut varmap_guard = state.varmap.lock().unwrap();
        *model_guard = Some(model);
        *varmap_guard = Some(varmap);
    }

    println!("--- Étape 2 : Entraînement (1 pas) ---");
    {
        let model_guard = state.model.lock().unwrap();
        let varmap_guard = state.varmap.lock().unwrap();

        if let (Some(model), Some(varmap)) = (&*model_guard, &*varmap_guard) {
            let trainer = Trainer::new(varmap, 0.1);
            let input = Tensor::randn(0f32, 1.0, (1, 1, input_dim), &device)?;
            let target = Tensor::zeros((1, 1), DType::U32, &device)?;

            let loss = trainer.train_step(model, &input, &target)?;
            println!("Loss intégration : {}", loss);
            assert!(loss > 0.0);
        } else {
            panic!("Le modèle aurait dû être initialisé !");
        }
    }

    println!("--- Étape 3 : Sauvegarde ---");
    {
        let varmap_guard = state.varmap.lock().unwrap();

        if let Some(varmap) = &*varmap_guard {
            serialization::save_model(varmap, &save_path)?;

            // Assertion stricte : le fichier DOIT exister ici
            if !save_path.exists() {
                panic!("❌ Le fichier n'a pas été créé : {:?}", save_path);
            }
        } else {
            panic!("❌ Erreur Étape 3 : VarMap est None.");
        }
    }

    println!("--- Étape 4 : Rechargement (Simulation redémarrage) ---");
    let new_state = DlState::new();
    {
        // On charge depuis le chemin unique
        let model =
            serialization::load_model(&save_path, input_dim, hidden_dim, output_dim, &device)?;

        let mut model_guard = new_state.model.lock().unwrap();
        let mut varmap_guard = new_state.varmap.lock().unwrap();

        *model_guard = Some(model);
        *varmap_guard = None;
    }

    println!("--- Étape 5 : Prédiction avec le modèle rechargé ---");
    {
        let model_guard = new_state.model.lock().unwrap();
        if let Some(model) = &*model_guard {
            let input = Tensor::randn(0f32, 1.0, (1, 1, input_dim), &device)?;
            let output = model.forward(&input)?;

            assert_eq!(output.dims(), &[1, 1, output_dim]);
            println!("Prédiction réussie : {:?}", output.to_vec3::<f32>()?);
        } else {
            panic!("Le modèle rechargé est introuvable !");
        }
    }

    // --- NETTOYAGE ROBUSTE ---
    // On essaie de supprimer, mais on ignore l'erreur si le fichier n'est plus là.
    if save_path.exists() {
        let _ = fs::remove_file(&save_path);
        println!("🗑️ Fichier temporaire nettoyé.");
    }

    Ok(())
}
