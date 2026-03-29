// FICHIER : src-tauri/src/commands/dl_commands.rs

use crate::utils::prelude::*;

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

// --- COMMANDES DEEP LEARNING ---

#[command]
pub fn init_dl_model(state: State<'_, DlState>) -> RaiseResult<String> {
    let config = &AppConfig::get().deep_learning;
    let device = config.to_device();

    let varmap = VarMap::new();
    let vb = VarBuilder::from_varmap(&varmap, DType::F32, &device);

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
            context = json_value!({
                "input_size": config.input_size,
                "hidden_size": config.hidden_size,
                "output_size": config.output_size
            })
        ),
    };

    *state.model.lock().unwrap() = Some(model);
    *state.varmap.lock().unwrap() = Some(varmap);
    Ok("OK".to_string())
}

#[command]
pub fn run_dl_prediction(state: State<'_, DlState>, input: Vec<f32>) -> RaiseResult<Vec<f32>> {
    let config = &AppConfig::get().deep_learning;
    let device = config.to_device();

    let guard = state.model.lock().unwrap();
    if let Some(model) = &*guard {
        // 🎯 FIX: Typage explicite (1usize) pour que le compilateur déduise la Shape
        let t = match Tensor::from_vec(input.clone(), (1usize, 1usize, config.input_size), &device)
        {
            Ok(tensor) => tensor,
            Err(e) => raise_error!(
                "ERR_MODEL_INPUT_TENSOR",
                error = e.to_string(),
                context = json_value!({ "expected_shape": [1, 1, config.input_size] })
            ),
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
        raise_error!("ERR_MODEL_NOT_LOADED", error = "MODEL_GUARD_IS_NONE");
    }
}

#[command]
pub fn train_dl_step(state: State<'_, DlState>, input: Vec<f32>, target: u32) -> RaiseResult<f64> {
    let config = &AppConfig::get().deep_learning;
    let device = config.to_device();

    let mg = state.model.lock().unwrap();
    let vg = state.varmap.lock().unwrap();

    if let (Some(model), Some(vars)) = (&*mg, &*vg) {
        let t_in = match Tensor::from_vec(input, (1usize, 1usize, config.input_size), &device) {
            Ok(t) => t,
            Err(e) => raise_error!("ERR_TRAIN_INPUT_TENSOR", error = e.to_string()),
        };

        let t_tgt = match Tensor::from_vec(vec![target], (1usize, 1usize), &device) {
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
    } else {
        raise_error!(
            "ERR_TRAIN_COMPONENTS_MISSING",
            error = "MODEL_OR_VARS_UNSET"
        );
    }
}

#[command]
pub fn save_dl_model(state: State<'_, DlState>, path: String) -> RaiseResult<String> {
    let vg = state.varmap.lock().unwrap();
    if let Some(vars) = &*vg {
        let path_buf = PathBuf::from(path);
        let path_display = path_buf.to_string_lossy().to_string();

        if let Err(e) = serialization::save_model(vars, path_buf) {
            raise_error!(
                "ERR_MODEL_SAVE_FAILURE",
                error = e.to_string(),
                context = json_value!({"path": path_display})
            );
        }

        Ok(format!("Model successfully saved to {}", path_display))
    } else {
        raise_error!("ERR_MODEL_SAVE_EMPTY", error = "NO_VARIABLES_IN_GUARD");
    }
}

#[command]
pub fn load_dl_model(state: State<'_, DlState>, path: String) -> RaiseResult<String> {
    let config = &AppConfig::get().deep_learning;

    let m = match serialization::load_model(PathBuf::from(path.clone()), config) {
        Ok(model) => model,
        Err(e) => raise_error!(
            "ERR_DL_MODEL_LOAD_FAIL",
            error = e.to_string(),
            context = json_value!({"path": path})
        ),
    };

    let mut model_guard = match state.model.lock() {
        Ok(guard) => guard,
        Err(_) => raise_error!(
            "ERR_SYS_MUTEX_POISONED",
            context = json_value!({"component": "DlState.model"})
        ),
    };

    let mut varmap_guard = match state.varmap.lock() {
        Ok(guard) => guard,
        Err(_) => raise_error!(
            "ERR_SYS_MUTEX_POISONED",
            context = json_value!({"component": "DlState.varmap"})
        ),
    };

    *model_guard = Some(m);
    *varmap_guard = None;

    Ok("Loaded".to_string())
}
