// FICHIER : crates/raise-desktop/src/commands/dl_commands.rs

use raise_core::ai::deep_learning::trainer::DeepLearningConfig;
use raise_core::utils::prelude::*;

// 🎯 On importe l'état et le service depuis le noyau
use raise_core::services::dl_service;
pub use raise_core::services::dl_service::DlState;

use tauri::{command, State};

#[command]
pub fn init_dl_model(state: State<'_, DlState>, config: DeepLearningConfig) -> RaiseResult<String> {
    dl_service::init_dl_model(state.inner(), config)
}

#[command]
pub fn run_dl_prediction(state: State<'_, DlState>, input: Vec<f32>) -> RaiseResult<Vec<f32>> {
    dl_service::run_dl_prediction(state.inner(), input)
}

#[command]
pub fn train_dl_step(state: State<'_, DlState>, input: Vec<f32>, target: u32) -> RaiseResult<f64> {
    dl_service::train_dl_step(state.inner(), input, target)
}

#[command]
pub fn save_dl_model(state: State<'_, DlState>, path: String) -> RaiseResult<String> {
    dl_service::save_dl_model(state.inner(), path)
}

#[command]
pub fn load_dl_model(
    state: State<'_, DlState>,
    path: String,
    config: DeepLearningConfig,
) -> RaiseResult<String> {
    dl_service::load_dl_model(state.inner(), path, config)
}
