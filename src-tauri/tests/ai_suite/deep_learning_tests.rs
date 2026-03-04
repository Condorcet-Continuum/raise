// FICHIER : src-tauri/tests/ai_suite/deep_learning_tests.rs

use candle_core::{DType, Tensor};
use candle_nn::{VarBuilder, VarMap};
use raise::ai::deep_learning::models::sequence_net::SequenceNet;
use raise::ai::deep_learning::serialization;
use raise::ai::deep_learning::trainer::Trainer;
use raise::commands::ai_commands::DlState;
use raise::utils::config::AppConfig;
use raise::utils::mock;
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

#[tokio::test]
async fn test_dl_e2e_integration() -> anyhow::Result<()> {
    // --- 1. CONFIGURATION ROBUSTE & ISOLÉE ---
    // 🎯 Inject the mock configuration to guarantee a stable testing environment
    mock::inject_mock_config().await;
    let config = &AppConfig::get().deep_learning;
    let device = config.to_device(); // Automatically resolves to CPU per the mock config

    let state = DlState::new();

    let start = SystemTime::now();
    let since_the_epoch = start
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards");
    let unique_id = since_the_epoch.as_nanos();

    let filename = format!("test_integration_model_{}.safetensors", unique_id);
    let save_path = std::env::temp_dir().join(filename);

    println!("📝 Fichier de test temporaire : {:?}", save_path);

    // 🎯 We no longer define input_dim, hidden_dim, output_dim here.
    // We use config.input_size, config.hidden_size, config.output_size.

    println!("--- Étape 1 : Initialisation du Modèle dans le State ---");
    {
        let varmap = VarMap::new();
        let vb = VarBuilder::from_varmap(&varmap, DType::F32, &device);
        // 🎯 Use configuration values
        let model = SequenceNet::new(
            config.input_size,
            config.hidden_size,
            config.output_size,
            vb,
        )?;

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
            // 🎯 Use the intelligent from_config constructor
            let trainer = Trainer::from_config(varmap, config);
            let input = Tensor::randn(0f32, 1.0, (1, 1, config.input_size), &device)?;
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
        // 🎯 FIX: Pass only the path and the config object
        let model = serialization::load_model(&save_path, config)?;

        let mut model_guard = new_state.model.lock().unwrap();
        let mut varmap_guard = new_state.varmap.lock().unwrap();

        *model_guard = Some(model);
        *varmap_guard = None;
    }

    println!("--- Étape 5 : Prédiction avec le modèle rechargé ---");
    {
        let model_guard = new_state.model.lock().unwrap();
        if let Some(model) = &*model_guard {
            let input = Tensor::randn(0f32, 1.0, (1, 1, config.input_size), &device)?;
            let output = model.forward(&input)?;

            // 🎯 Verify against the configured output size
            assert_eq!(output.dims(), &[1, 1, config.output_size]);
            println!("Prédiction réussie : {:?}", output.to_vec3::<f32>()?);
        } else {
            panic!("Le modèle rechargé est introuvable !");
        }
    }

    if save_path.exists() {
        let _ = fs::remove_file(&save_path);
        println!("🗑️ Fichier temporaire nettoyé.");
    }

    Ok(())
}
