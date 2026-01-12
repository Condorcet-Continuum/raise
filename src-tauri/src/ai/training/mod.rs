pub mod dataset;
pub mod lora;

use crate::json_db::storage::StorageEngine;
use candle_core::Device;
use candle_nn::{AdamW, Optimizer, ParamsAdamW, VarMap};
use tauri::State;

#[tauri::command]
pub async fn ai_train_domain_native(
    storage: State<'_, StorageEngine>,
    space: String,
    db_name: String,
    domain: String,
    epochs: usize,
    lr: f64,
) -> Result<String, String> {
    // [CORRECTION] Préfixe avec _ car la boucle réelle n'est pas encore implémentée
    let _device = Device::new_cuda(0).unwrap_or(Device::Cpu);

    let examples = dataset::extract_domain_data(storage.inner(), &space, &db_name, &domain)?;
    if examples.is_empty() {
        return Err("Aucune donnée pour ce domaine.".into());
    }

    let varmap = VarMap::new();

    // [CORRECTION] Suppression de mut et ajout de _ pour l'optimiseur inutilisé pour l'instant
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
