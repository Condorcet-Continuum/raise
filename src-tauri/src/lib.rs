pub mod blockchain;
pub mod commands;
pub mod json_db;
pub mod model_engine;

pub mod ai;
pub mod code_generator;
pub mod plugins;

pub mod traceability;

use crate::model_engine::types::ProjectModel;
use std::sync::Mutex;

// Cette structure rend l'état accessible partout via crate::AppState
pub struct AppState {
    pub model: Mutex<ProjectModel>,
}
