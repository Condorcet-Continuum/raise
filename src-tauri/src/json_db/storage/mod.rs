//! Moteur de stockage sur disque

pub mod file_storage;
pub mod cache;
pub mod compression;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    pub data_dir: String,
    pub cache_size_mb: usize,
    pub compression_enabled: bool,
    pub auto_compact: bool,
}
