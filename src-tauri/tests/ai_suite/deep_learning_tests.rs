// FICHIER : src-tauri/tests/ai_suite/deep_learning_tests.rs

use candle_core::{DType, Device, Tensor};
use candle_nn::{VarBuilder, VarMap};
use raise::ai::deep_learning::models::sequence_net::SequenceNet;
use raise::ai::deep_learning::serialization;
use raise::ai::deep_learning::trainer::Trainer;
use raise::commands::ai_commands::DlState;
use std::fs;
use std::path::PathBuf;

#[test] // CORRECTION : Retour au test synchrone standard
fn test_dl_e2e_integration() -> anyhow::Result<()> {
    // 1. Initialisation de l'État Global (Simule le démarrage de l'app)
    let state = DlState::new();
    let device = Device::Cpu;
    let save_path = PathBuf::from("test_integration_model.safetensors");

    // Hyperparamètres
    let input_dim = 5;
    let hidden_dim = 10;
    let output_dim = 2;

    println!("--- Étape 1 : Initialisation du Modèle dans le State ---");
    {
        // On simule ce que fait `init_dl_model`
        let varmap = VarMap::new();
        let vb = VarBuilder::from_varmap(&varmap, DType::F32, &device);
        let model = SequenceNet::new(input_dim, hidden_dim, output_dim, vb)?;

        // CORRECTION E0277 : lock() renvoie un Result (std::sync::Mutex).
        // On retire .await et on utilise .unwrap() comme suggéré par le compilateur.
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

            // Données factices [1, 1, 5]
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
            assert!(
                save_path.exists(),
                "Le fichier de sauvegarde n'a pas été créé"
            );
        }
    }

    println!("--- Étape 4 : Rechargement (Simulation redémarrage) ---");
    // On crée un NOUVEL état pour simuler un redémarrage complet de l'application
    let new_state = DlState::new();
    {
        // Simulation de `load_dl_model`
        let model =
            serialization::load_model(&save_path, input_dim, hidden_dim, output_dim, &device)?;

        let mut model_guard = new_state.model.lock().unwrap();
        let mut varmap_guard = new_state.varmap.lock().unwrap();

        *model_guard = Some(model);
        *varmap_guard = None; // Mode inférence (pas de varmap)
    }

    println!("--- Étape 5 : Prédiction avec le modèle rechargé ---");
    {
        let model_guard = new_state.model.lock().unwrap();
        if let Some(model) = &*model_guard {
            let input = Tensor::randn(0f32, 1.0, (1, 1, input_dim), &device)?;
            let output = model.forward(&input)?;

            // Vérification de la forme de sortie [1, 1, output_dim]
            assert_eq!(output.dims(), &[1, 1, output_dim]);
            println!("Prédiction réussie : {:?}", output.to_vec3::<f32>()?);
        } else {
            panic!("Le modèle rechargé est introuvable !");
        }
    }

    // Nettoyage
    if save_path.exists() {
        fs::remove_file(save_path)?;
    }

    Ok(())
}
