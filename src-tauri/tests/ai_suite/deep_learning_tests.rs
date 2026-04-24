// FICHIER : src-tauri/tests/ai_suite/deep_learning_tests.rs
use raise::utils::prelude::*; // 🎯 Façade Unique RAISE

use raise::ai::deep_learning::models::sequence_net::SequenceNet;
use raise::ai::deep_learning::serialization;
use raise::ai::deep_learning::trainer::DeepLearningConfig;
use raise::ai::deep_learning::trainer::Trainer;
use raise::commands::dl_commands::DlState;
use raise::utils::testing::*;
use std::time::{SystemTime, UNIX_EPOCH};

fn get_test_dl_config() -> DeepLearningConfig {
    DeepLearningConfig {
        learning_rate: 0.01,
        input_size: 5,
        hidden_size: 10,
        output_size: 2,
    }
}

#[async_test]
#[serial_test::serial]
#[cfg_attr(not(feature = "cuda"), ignore)]
async fn test_dl_e2e_integration() -> RaiseResult<()> {
    // --- 1. CONFIGURATION ROBUSTE & ISOLÉE ---
    mock::inject_mock_config().await;
    let config = get_test_dl_config();

    let device = ComputeHardware::Cpu;

    let state = DlState::new();
    let start = SystemTime::now();
    let unique_id = match start.duration_since(UNIX_EPOCH) {
        Ok(d) => d.as_nanos(),
        Err(_) => 0,
    };

    let filename = format!("test_integration_model_{}.safetensors", unique_id);
    let save_path = std::env::temp_dir().join(filename);

    user_info!(
        "INF_DL_TEST_START",
        json_value!({"path": save_path.to_string_lossy()})
    );

    // --- Étape 1 : Initialisation du Modèle (Match strict) ---
    {
        let varmap = NeuralWeightsMap::new();
        let vb = NeuralWeightsBuilder::from_varmap(&varmap, ComputeType::F32, &device);

        let model = match SequenceNet::new(
            config.input_size,
            config.hidden_size,
            config.output_size,
            vb,
        ) {
            Ok(m) => m,
            Err(e) => raise_error!("ERR_DL_MODEL_INIT", error = e.to_string()),
        };

        // Verrouillage sécurisé sans map_err
        match (state.model.lock(), state.varmap.lock()) {
            (Ok(mut m_guard), Ok(mut v_guard)) => {
                *m_guard = Some(model);
                *v_guard = Some(varmap);
            }
            _ => raise_error!("ERR_DL_LOCK_POISONED"),
        }
    }

    // --- Étape 2 : Entraînement (Zéro map_err / Portée restreinte) ---
    {
        // On récupère les gardes dans un bloc restreint pour éviter les problèmes de Send sur await
        let (model_opt, varmap_opt) = match (state.model.lock(), state.varmap.lock()) {
            (Ok(m), Ok(v)) => (m, v),
            _ => raise_error!("ERR_DL_LOCK_POISONED"),
        };

        match (&*model_opt, &*varmap_opt) {
            (Some(model), Some(varmap)) => {
                let mut trainer = match Trainer::from_config(varmap, &config) {
                    Ok(t) => t,
                    Err(e) => raise_error!("ERR_DL_TRAINER_CONFIG", error = e.to_string()),
                };

                let input = match NeuralTensor::randn(0f32, 1.0, (1, 1, config.input_size), &device)
                {
                    Ok(t) => t,
                    Err(e) => raise_error!("ERR_DL_TENSOR_GEN", error = e.to_string()),
                };

                let target = match NeuralTensor::zeros((1, 1), ComputeType::U32, &device) {
                    Ok(t) => t,
                    Err(e) => raise_error!("ERR_DL_TARGET_GEN", error = e.to_string()),
                };

                let loss = trainer.train_step(model, &input, &target)?;
                assert!(loss > 0.0);
                user_success!("SUC_DL_TRAIN_STEP", json_value!({"loss": loss}));
            }
            _ => raise_error!("ERR_DL_UNINITIALIZED_STATE"),
        }
    }

    // --- Étape 3 : Sauvegarde ---
    {
        let varmap_guard = match state.varmap.lock() {
            Ok(g) => g,
            Err(_) => raise_error!("ERR_DL_LOCK_POISONED"),
        };

        match &*varmap_guard {
            Some(varmap) => match serialization::save_model(varmap, &save_path) {
                Ok(_) => {
                    if !save_path.exists() {
                        raise_error!("ERR_DL_SAVE_MISSING_FILE");
                    }
                }
                Err(e) => raise_error!("ERR_DL_SAVE_FAIL", error = e.to_string()),
            },
            None => raise_error!("ERR_DL_VARMAP_NONE"),
        }
    }

    // --- Étape 4 : Rechargement ---
    let new_state = DlState::new();
    {
        let model = match serialization::load_model(&save_path, &config) {
            Ok(m) => m,
            Err(e) => raise_error!("ERR_DL_LOAD_FAIL", error = e.to_string()),
        };

        match new_state.model.lock() {
            Ok(mut m_guard) => *m_guard = Some(model),
            Err(_) => raise_error!("ERR_DL_LOCK_POISONED"),
        }
    }

    if save_path.exists() {
        let _ = fs::remove_file_sync(&save_path);
    }

    user_success!("SUC_DL_E2E_VALIDATED");
    Ok(())
}

// =========================================================================
// NOUVEAUX TESTS : RÉSILIENCE ET CONCURRENCE
// =========================================================================

#[cfg(test)]
mod resilience_tests {
    use super::*;

    /// 🎯 Test la résilience face à des dimensions de configuration invalides (Match strict)
    #[async_test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_dl_config_dimension_resilience() -> RaiseResult<()> {
        mock::inject_mock_config().await;
        let mut config = get_test_dl_config();
        config.input_size = 0; // Injection d'erreur

        let device = ComputeHardware::Cpu;
        let varmap = NeuralWeightsMap::new();
        let vb = NeuralWeightsBuilder::from_varmap(&varmap, ComputeType::F32, &device);

        match SequenceNet::new(
            config.input_size,
            config.hidden_size,
            config.output_size,
            vb,
        ) {
            Err(_) => Ok(()),
            Ok(_) => panic!("Le moteur aurait dû rejeter une dimension nulle"),
        }
    }

    /// 🎯 Test la protection des verrous (Mutex) en cas de charge asynchrone (Correction Send)
    #[async_test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_dl_state_concurrency_resilience() -> RaiseResult<()> {
        let state = SharedRef::new(DlState::new());
        let mut handles = vec![];

        for _ in 0..5 {
            let state_clone = state.clone();
            handles.push(tokio::spawn(async move {
                // 🎯 FIX : Bloc de portée restreinte pour libérer le verrou AVANT le .await
                {
                    match state_clone.model.lock() {
                        Ok(_lock) => { /* travail court */ }
                        Err(_) => panic!("Lock poisoned"),
                    }
                }
                // Le verrou est relâché ici, l'await est désormais "Send-Safe"
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }));
        }

        for h in handles {
            match h.await {
                Ok(_) => (),
                Err(e) => raise_error!("ERR_DL_JOIN_FAIL", error = e.to_string()),
            }
        }

        user_success!("SUC_DL_CONCURRENCY_OK");
        Ok(())
    }
}
