pub mod blockchain;
pub mod commands;
pub mod json_db;
pub mod model_engine;
pub mod rules_engine;

pub mod ai;
pub mod code_generator;
pub mod plugins;

pub mod traceability;

pub mod utils;
pub mod workflow_engine;

pub mod genetics;
pub mod graph_store;

pub mod spatial_engine;

use crate::model_engine::types::ProjectModel;
use std::sync::Mutex;

pub struct AppState {
    pub model: Mutex<ProjectModel>,
}
