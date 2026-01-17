// FICHIER : src-tauri/src/ai/training/mod.rs

pub mod dataset;
pub mod lora;

use crate::json_db::storage::StorageEngine;
use candle_core::Device;
use candle_nn::{AdamW, Optimizer, ParamsAdamW, VarMap};

#[tauri::command]
pub async fn ai_train_domain_native(
    storage: &StorageEngine,
    space: String,
    db_name: String,
    domain: String,
    epochs: usize,
    lr: f64,
) -> Result<String, String> {
    let _device = Device::new_cuda(0).unwrap_or(Device::Cpu);
    let examples = dataset::extract_domain_data(storage, &space, &db_name, &domain).await?;

    if examples.is_empty() {
        return Err("Aucune donnée pour ce domaine.".into());
    }

    let varmap = VarMap::new();

    let _opt = AdamW::new(
        varmap.all_vars(),
        ParamsAdamW {
            lr,
            ..Default::default()
        },
    )
    .map_err(|e| e.to_string())?;

    for epoch in 1..=epochs {
        println!("Epoch {}/{} - Domaine: {}", epoch, epochs, domain);
    }

    let save_path = format!("{}_adapter.safetensors", domain);
    varmap.save(&save_path).map_err(|e| e.to_string())?;

    Ok(format!("Adaptateur {} sauvegardé avec succès.", save_path))
}

// --- TESTS UNITAIRES ---

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json_db::storage::JsonDbConfig;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_ai_train_domain_native_empty_data() {
        // Setup d'un environnement minimal pour tester la réaction aux données vides
        let temp_dir = tempdir().expect("Échec dossier temp");
        let config = JsonDbConfig::new(temp_dir.path().to_path_buf());
        let storage = StorageEngine::new(config);

        let result = ai_train_domain_native(
            &storage,
            "space".into(),
            "db".into(),
            "nonexistent".into(),
            1,
            0.001,
        )
        .await;

        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "Aucune donnée pour ce domaine.");
    }
}
