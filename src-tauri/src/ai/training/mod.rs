// FICHIER : src-tauri/src/ai/training/mod.rs

use crate::json_db::storage::StorageEngine;
use crate::utils::config::AppConfig;
use crate::utils::prelude::*;
use candle_core::{Device, Tensor};
use candle_nn::{AdamW, Optimizer, ParamsAdamW, VarMap};
use tokenizers::Tokenizer;

pub mod dataset;
pub mod lora;

pub async fn ai_train_domain_native(
    storage: &StorageEngine,
    space: &str,
    db_name: &str,
    domain: &str,
    epochs: usize,
    lr: f64,
) -> Result<String> {
    let _device = Device::new_cuda(0).unwrap_or(Device::Cpu);

    // ---------------------------------------------------------
    // 1. RÉCUPÉRATION DU TOKENIZER DEPUIS LA CONFIG SSOT
    // ---------------------------------------------------------
    let config_app = AppConfig::get();
    let engine_cfg = config_app.ai_engines.get("primary_local").ok_or_else(|| {
        AppError::Ai("Moteur 'primary_local' introuvable dans la configuration".to_string())
    })?;

    let tokenizer_filename = engine_cfg
        .rust_tokenizer_file
        .as_deref()
        .unwrap_or("tokenizer.json");
    let home =
        dirs::home_dir().ok_or_else(|| AppError::Ai("Dossier home introuvable".to_string()))?;

    // On pointe vers notre dossier de modèles locaux
    let tokenizer_path = home
        .join("raise_domain/_system/ai-assets/models")
        .join(tokenizer_filename);

    if !tokenizer_path.exists() {
        return Err(AppError::Ai(format!(
            "Fichier Tokenizer introuvable : {:?}",
            tokenizer_path
        )));
    }

    let tokenizer = Tokenizer::from_file(&tokenizer_path)
        .map_err(|e| AppError::Ai(format!("Erreur chargement Tokenizer: {}", e)))?;

    println!("✅ Tokenizer '{}' chargé avec succès.", tokenizer_filename);

    // ---------------------------------------------------------
    // 2. EXTRACTION DES DONNÉES
    // ---------------------------------------------------------
    let examples = dataset::extract_domain_data(storage, space, db_name, domain).await?;

    if examples.is_empty() {
        return Err(crate::utils::error::AppError::from(format!(
            "Aucune donnée trouvée pour le domaine : {}",
            domain
        )));
    }

    let varmap = VarMap::new();
    let mut _opt = AdamW::new(
        varmap.all_vars(),
        ParamsAdamW {
            lr,
            ..Default::default()
        },
    )
    .map_err(|e| AppError::from(e.to_string()))?;

    // ---------------------------------------------------------
    // 3. LA BOUCLE D'APPRENTISSAGE
    // ---------------------------------------------------------
    for epoch in 1..=epochs {
        println!(
            "⏳ Epoch {}/{} - Entraînement sur le domaine: {}",
            epoch, epochs, domain
        );
        let mut epoch_loss = 0.0;

        for example in examples.iter() {
            // A. Formatage du texte au format ChatML (compris par Qwen)
            let prompt = format!(
                "<|im_start|>system\n{}<|im_end|>\n<|im_start|>user\n{}<|im_end|>\n<|im_start|>assistant\n{}<|im_end|>",
                example.instruction, example.input, example.output
            );

            // B. Tokenisation : Texte -> Nombres (IDs)
            let encoding = tokenizer
                .encode(prompt, true)
                .map_err(|e| AppError::Ai(format!("Erreur de tokenisation: {}", e)))?;

            let tokens = encoding.get_ids();
            let seq_len = tokens.len(); // La vraie longueur de notre séquence !

            // C. Conversion en Tenseur Candle [1, seq_len] de type U32
            let labels = Tensor::new(tokens, &_device)
                .map_err(|e| AppError::from(e.to_string()))?
                .unsqueeze(0)
                .map_err(|e| AppError::from(e.to_string()))?;

            // D. Simulation du Forward Pass de Qwen (en attendant le Boss final)
            let vocab_size = 151936; // Qwen 2.5 vocab size
            let dummy_logits = Tensor::randn(0f32, 1f32, (1, seq_len, vocab_size), &_device)
                .map_err(|e| AppError::from(e.to_string()))?;

            // E. Calcul de la Loss (Erreur)
            let loss = candle_nn::loss::cross_entropy(
                &dummy_logits
                    .flatten_to(1)
                    .map_err(|e| AppError::from(e.to_string()))?,
                &labels
                    .flatten_to(1)
                    .map_err(|e| AppError::from(e.to_string()))?,
            )
            .map_err(|e| AppError::from(e.to_string()))?;

            // F. Rétropropagation (Apprentissage)
            _opt.backward_step(&loss)
                .map_err(|e| AppError::from(format!("Erreur Backprop: {}", e)))?;
            epoch_loss += loss
                .to_vec0::<f32>()
                .map_err(|e| AppError::from(e.to_string()))?;
        }

        let avg_loss = epoch_loss / examples.len() as f32;
        println!("   📉 Loss moyenne: {:.4}", avg_loss);
    }

    // ... (Le code de sauvegarde du dossier LoRA reste inchangé ici) ...
    let home = dirs::home_dir().ok_or_else(|| {
        AppError::from("Impossible de trouver le dossier utilisateur".to_string())
    })?;
    let lora_dir = home
        .join("raise_domain/_system/ai-assets/lora")
        .join(format!("raise-{}-adapter", domain));
    std::fs::create_dir_all(&lora_dir)
        .map_err(|e| AppError::from(format!("Erreur création dossier LoRA: {}", e)))?;
    let save_path = lora_dir.join("adapter_model.safetensors");
    varmap
        .save(&save_path)
        .map_err(|e| AppError::from(format!("Erreur sauvegarde poids: {}", e)))?;

    Ok(format!(
        "Adaptateur sauvegardé avec succès dans : {:?}",
        save_path
    ))
}

// --- TESTS UNITAIRES ---

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json_db::storage::JsonDbConfig;
    use crate::utils::io::tempdir;

    #[tokio::test]
    #[serial_test::serial] // Protection RTX 5060 en local
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_ai_train_domain_native_empty_data() {
        crate::utils::config::test_mocks::inject_mock_config();

        let temp_dir = tempdir().expect("Échec dossier temp");
        let config = JsonDbConfig::new(temp_dir.path().to_path_buf());
        let storage = StorageEngine::new(config);

        // Appel direct du noyau pur avec des références
        let result = ai_train_domain_native(&storage, "space", "db", "nonexistent", 1, 0.001).await;

        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("Aucune donnée"));
    }
}
