// FICHIER : src-tauri/src/commands/dl_commands.rs

use crate::utils::prelude::*; // 🎯 Façade Unique RAISE

// Imports Deep Learning
use crate::ai::deep_learning::{
    models::sequence_net::SequenceNet, serialization, trainer::Trainer,
};
use candle_core::{DType, Tensor};
use candle_nn::{VarBuilder, VarMap};
use tauri::{command, State};

// --- STATE ---
pub struct DlState {
    pub model: SyncMutex<Option<SequenceNet>>,
    pub varmap: SyncMutex<Option<VarMap>>,
}

impl DlState {
    pub fn new() -> Self {
        Self {
            model: SyncMutex::new(None),
            varmap: SyncMutex::new(None),
        }
    }
}

impl Default for DlState {
    fn default() -> Self {
        Self::new()
    }
}

// --- COMMANDES DEEP LEARNING (INTERFACE TAURI) ---

#[command]
pub fn init_dl_model(state: State<'_, DlState>) -> RaiseResult<String> {
    init_dl_model_internal(&state)
}

#[command]
pub fn run_dl_prediction(state: State<'_, DlState>, input: Vec<f32>) -> RaiseResult<Vec<f32>> {
    run_dl_prediction_internal(&state, input)
}

#[command]
pub fn train_dl_step(state: State<'_, DlState>, input: Vec<f32>, target: u32) -> RaiseResult<f64> {
    train_dl_step_internal(&state, input, target)
}

#[command]
pub fn save_dl_model(state: State<'_, DlState>, path: String) -> RaiseResult<String> {
    save_dl_model_internal(&state, path)
}

#[command]
pub fn load_dl_model(state: State<'_, DlState>, path: String) -> RaiseResult<String> {
    load_dl_model_internal(&state, path)
}

// --- LOGIQUE INTERNE (TESTABLE SANS TAURI::STATE) ---

fn init_dl_model_internal(state: &DlState) -> RaiseResult<String> {
    let config = &AppConfig::get().deep_learning;
    let device = AppConfig::device(); // 🎯 Façade SSOT

    let varmap = VarMap::new();
    let vb = VarBuilder::from_varmap(&varmap, DType::F32, device);

    let model = match SequenceNet::new(
        config.input_size,
        config.hidden_size,
        config.output_size,
        vb,
    ) {
        Ok(m) => m,
        Err(e) => raise_error!(
            "ERR_AI_MODEL_INIT_FAIL",
            error = e.to_string(),
            context = json_value!({ "input_size": config.input_size })
        ),
    };

    let mut mg = match state.model.lock() {
        Ok(g) => g,
        Err(_) => raise_error!("ERR_SYS_MUTEX_POISONED"),
    };
    let mut vg = match state.varmap.lock() {
        Ok(g) => g,
        Err(_) => raise_error!("ERR_SYS_MUTEX_POISONED"),
    };

    *mg = Some(model);
    *vg = Some(varmap);
    Ok("OK".to_string())
}

fn run_dl_prediction_internal(state: &DlState, input: Vec<f32>) -> RaiseResult<Vec<f32>> {
    let config = &AppConfig::get().deep_learning;
    let device = AppConfig::device();

    let guard = match state.model.lock() {
        Ok(g) => g,
        Err(_) => raise_error!("ERR_SYS_MUTEX_POISONED"),
    };

    if let Some(model) = &*guard {
        let t = match Tensor::from_vec(input, (1usize, 1usize, config.input_size), device) {
            Ok(tensor) => tensor,
            Err(e) => raise_error!("ERR_MODEL_INPUT_TENSOR", error = e.to_string()),
        };

        let out = match model.forward(&t) {
            Ok(output) => output,
            Err(e) => raise_error!("ERR_MODEL_FORWARD_PASS", error = e.to_string()),
        };

        match out.flatten_all().and_then(|o| o.to_vec1::<f32>()) {
            Ok(vec) => Ok(vec),
            Err(e) => raise_error!("ERR_MODEL_OUTPUT_CONVERSION", error = e.to_string()),
        }
    } else {
        raise_error!("ERR_MODEL_NOT_LOADED")
    }
}

fn train_dl_step_internal(state: &DlState, input: Vec<f32>, target: u32) -> RaiseResult<f64> {
    let config = &AppConfig::get().deep_learning;
    let device = AppConfig::device();

    let mg = match state.model.lock() {
        Ok(g) => g,
        Err(_) => raise_error!("ERR_SYS_MUTEX_POISONED"),
    };
    let vg = match state.varmap.lock() {
        Ok(g) => g,
        Err(_) => raise_error!("ERR_SYS_MUTEX_POISONED"),
    };

    match (&*mg, &*vg) {
        (Some(model), Some(vars)) => {
            let t_in = match Tensor::from_vec(input, (1usize, 1usize, config.input_size), device) {
                Ok(t) => t,
                Err(e) => raise_error!("ERR_TRAIN_INPUT_TENSOR", error = e.to_string()),
            };

            let t_tgt = match Tensor::from_vec(vec![target], (1usize, 1usize), device) {
                Ok(t) => t,
                Err(e) => raise_error!("ERR_TRAIN_TARGET_TENSOR", error = e.to_string()),
            };

            let mut trainer = match Trainer::from_config(vars, config) {
                Ok(t) => t,
                Err(e) => raise_error!("ERR_TRAINER_INIT", error = e.to_string()),
            };

            match trainer.train_step(model, &t_in, &t_tgt) {
                Ok(loss) => Ok(loss),
                Err(e) => raise_error!("ERR_TRAIN_STEP_FAILURE", error = e.to_string()),
            }
        }
        _ => raise_error!("ERR_TRAIN_COMPONENTS_MISSING"),
    }
}

fn save_dl_model_internal(state: &DlState, path: String) -> RaiseResult<String> {
    let vg = match state.varmap.lock() {
        Ok(g) => g,
        Err(_) => raise_error!("ERR_SYS_MUTEX_POISONED"),
    };

    if let Some(vars) = &*vg {
        match serialization::save_model(vars, PathBuf::from(&path)) {
            Ok(_) => Ok(format!("Model saved to {}", path)),
            Err(e) => raise_error!("ERR_MODEL_SAVE_FAILURE", error = e.to_string()),
        }
    } else {
        raise_error!("ERR_MODEL_SAVE_EMPTY")
    }
}

fn load_dl_model_internal(state: &DlState, path: String) -> RaiseResult<String> {
    let config = &AppConfig::get().deep_learning;
    let path_buf = PathBuf::from(&path);

    if !path_buf.exists() {
        raise_error!(
            "ERR_DL_MODEL_NOT_FOUND",
            context = json_value!({"path": path})
        );
    }

    let m = match serialization::load_model(path_buf, config) {
        Ok(model) => model,
        Err(e) => raise_error!("ERR_DL_MODEL_LOAD_FAIL", error = e.to_string()),
    };

    let mut model_guard = match state.model.lock() {
        Ok(guard) => guard,
        Err(_) => raise_error!("ERR_SYS_MUTEX_POISONED"),
    };

    *model_guard = Some(m);
    Ok("Loaded".to_string())
}

// =========================================================================
// TESTS UNITAIRES ET RÉSILIENCE
// =========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::testing::mock;

    #[async_test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_dl_commands_initialization() -> RaiseResult<()> {
        mock::inject_mock_config().await;
        let state = DlState::new();
        // 🎯 FIX: Appel de la logique interne au lieu de la commande Tauri
        let res = init_dl_model_internal(&state);
        assert!(res.is_ok());
        Ok(())
    }

    #[async_test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_resilience_uninitialized_prediction() -> RaiseResult<()> {
        // 🎯 FIX : Initialisation de la config globale pour éviter la panique sur AppConfig::get()
        mock::inject_mock_config().await;

        let state = DlState::new();
        // 🎯 FIX : Vérification que la prédiction échoue proprement si le modèle n'est pas chargé
        let res = run_dl_prediction_internal(&state, vec![0.1]);
        assert!(res.is_err());
        Ok(())
    }

    #[async_test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_dl_device_ssot() -> RaiseResult<()> {
        // 🎯 FIX : Initialisation du matériel pour éviter la panique sur AppConfig::device()
        mock::inject_mock_config().await;

        let device = AppConfig::device();
        // Le périphérique doit être valide pour le moteur Candle
        assert!(device.is_cpu() || device.is_cuda() || device.is_metal());
        Ok(())
    }
}
