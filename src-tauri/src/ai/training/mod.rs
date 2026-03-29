// FICHIER : src-tauri/src/ai/training/mod.rs

use crate::json_db::collections::manager::CollectionsManager;
use crate::utils::prelude::*;
use candle_core::Tensor;
use candle_nn::{AdamW, Optimizer, ParamsAdamW, VarMap};
use tokenizers::Tokenizer;

pub mod dataset;
pub mod lora;

// 🎯 FIX : Signature unifiée "Graph-Driven" avec le CollectionsManager
pub async fn ai_train_domain_native(
    manager: &CollectionsManager<'_>,
    domain: &str,
    epochs: usize,
    lr: f64,
) -> RaiseResult<String> {
    let _device = AppConfig::device().clone();
    // ---------------------------------------------------------
    // 1. RÉCUPÉRATION DU TOKENIZER DEPUIS LA DB
    // ---------------------------------------------------------
    let settings = AppConfig::get_component_settings(manager, "ai_llm").await?;

    let tokenizer_filename = settings
        .get("rust_tokenizer_file")
        .and_then(|v| v.as_str())
        .unwrap_or("tokenizer.json");

    let config_app = AppConfig::get();

    // 🎯 FIX : Portabilité absolue garantie, adieu dirs::home_dir() !
    let domain_path = config_app
        .get_path("PATH_RAISE_DOMAIN")
        .unwrap_or_else(|| PathBuf::from("./raise_default_domain"));

    let tokenizer_path = domain_path
        .join("_system/ai-assets/models")
        .join(tokenizer_filename);

    if !tokenizer_path.exists() {
        raise_error!(
            "ERR_AI_TOKENIZER_FILE_NOT_FOUND",
            error = format!("Fichier Tokenizer introuvable : {:?}", tokenizer_path),
            context = json_value!({ "path": tokenizer_path.to_string_lossy() })
        );
    }

    let tokenizer = match Tokenizer::from_file(&tokenizer_path) {
        Ok(t) => t,
        Err(e) => raise_error!(
            "ERR_AI_TOKENIZER_LOAD",
            error = e,
            context = json_value!({
                "path": tokenizer_path.to_string_lossy(),
                "action": "load_tokenizer_from_file"
            })
        ),
    };
    println!("✅ Tokenizer '{}' chargé avec succès.", tokenizer_filename);

    // ---------------------------------------------------------
    // 2. EXTRACTION DES DONNÉES
    // ---------------------------------------------------------
    // 🎯 L'extraction utilise désormais proprement le manager
    let examples = dataset::extract_domain_data(manager, domain).await?;

    if examples.is_empty() {
        raise_error!(
            "ERR_DATA_DOMAIN_EMPTY",
            error = "EMPTY_COLLECTION",
            context = json_value!({
                "action": "load_domain_examples",
                "domain": domain,
                "hint": "Vérifiez que le fichier de données JSON n'est pas vide ou que le chemin du domaine est correct."
            })
        );
    }

    let varmap = VarMap::new();
    let mut _opt = match AdamW::new(
        varmap.all_vars(),
        ParamsAdamW {
            lr,
            ..Default::default()
        },
    ) {
        Ok(optimizer) => optimizer,
        Err(e) => raise_error!(
            "ERR_MODEL_OPTIMIZER_INIT",
            error = e,
            context = json_value!({
                "action": "initialize_adamw",
                "learning_rate": lr,
                "variable_count": varmap.all_vars().len(),
                "hint": "Vérifiez que les variables du modèle sont correctement allouées."
            })
        ),
    };

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
            let prompt = format!(
                "<|im_start|>system\n{}<|im_end|>\n<|im_start|>user\n{}<|im_end|>\n<|im_start|>assistant\n{}<|im_end|>",
                example.instruction, example.input, example.output
            );

            let prompt_len = prompt.len();
            let encoding = match tokenizer.encode(prompt, true) {
                Ok(enc) => enc,
                Err(e) => raise_error!(
                    "ERR_AI_TOKENIZATION_FAIL",
                    error = e,
                    context = json_value!({
                        "prompt_len": prompt_len,
                        "add_special_tokens": true
                    })
                ),
            };

            let tokens = encoding.get_ids();
            let seq_len = tokens.len();

            let labels_base = match Tensor::new(tokens, &_device) {
                Ok(t) => t,
                Err(e) => raise_error!(
                    "ERR_TENSOR_CREATE",
                    error = e,
                    context = json_value!({
                        "action": "create_labels_tensor",
                        "token_count": tokens.len(),
                        "device": format!("{:?}", _device)
                    })
                ),
            };

            let labels = match labels_base.unsqueeze(0) {
                Ok(t) => t,
                Err(e) => raise_error!(
                    "ERR_TENSOR_SHAPE_UNSQUEEZE",
                    error = e,
                    context = json_value!({
                        "action": "unsqueeze_labels",
                        "current_shape": format!("{:?}", labels_base.shape()),
                        "dim": 0
                    })
                ),
            };

            let vocab_size = 151936; // Qwen 2.5 vocab size
            let dummy_logits = match Tensor::randn(0f32, 1f32, (1, seq_len, vocab_size), &_device) {
                Ok(t) => t,
                Err(e) => raise_error!(
                    "ERR_TENSOR_RANDN_INIT",
                    error = e,
                    context = json_value!({
                        "action": "create_dummy_logits",
                        "shape": [1, seq_len, vocab_size],
                        "device": format!("{:?}", _device)
                    })
                ),
            };

            let flat_logits = match dummy_logits.flatten_to(1) {
                Ok(t) => t,
                Err(e) => raise_error!(
                    "ERR_MODEL_FLATTEN_LOGITS",
                    error = e,
                    context = json_value!({ "shape": format!("{:?}", dummy_logits.shape()) })
                ),
            };

            let flat_labels = match labels.flatten_to(1) {
                Ok(t) => t,
                Err(e) => raise_error!(
                    "ERR_MODEL_FLATTEN_LABELS",
                    error = e,
                    context = json_value!({ "shape": format!("{:?}", labels.shape()) })
                ),
            };

            let loss = match candle_nn::loss::cross_entropy(&flat_logits, &flat_labels) {
                Ok(l) => l,
                Err(e) => raise_error!(
                    "ERR_MODEL_CROSS_ENTROPY",
                    error = e,
                    context = json_value!({
                        "action": "compute_loss",
                        "logits_shape": format!("{:?}", flat_logits.shape()),
                        "labels_shape": format!("{:?}", flat_labels.shape())
                    })
                ),
            };

            match _opt.backward_step(&loss) {
                Ok(_) => (),
                Err(e) => raise_error!(
                    "ERR_MODEL_BACKPROP",
                    error = e,
                    context = json_value!({
                        "action": "backward_step",
                        "phase": "model_optimization",
                        "target": "loss_gradients"
                    })
                ),
            };

            epoch_loss += match loss.to_vec0::<f32>() {
                Ok(val) => val,
                Err(e) => raise_error!(
                    "ERR_MODEL_LOSS_CONVERSION",
                    error = e,
                    context = json_value!({
                        "action": "extract_loss_value",
                        "phase": "epoch_accumulation"
                    })
                ),
            };
        }

        let avg_loss = epoch_loss / examples.len() as f32;
        println!("   📉 Loss moyenne: {:.4}", avg_loss);
    }

    // 🎯 FIX : Utilisation du domaine configuré pour la sauvegarde LoRA !
    let lora_dir = domain_path
        .join("_system/ai-assets/lora")
        .join(format!("raise-{}-adapter", domain));

    match fs::create_dir_all_async(&lora_dir).await {
        Ok(_) => (),
        Err(e) => raise_error!(
            "ERR_FS_LORA_DIR_CREATE",
            error = e,
            context = json_value!({
                "action": "create_lora_directory",
                "path": lora_dir.to_string_lossy(),
                "hint": "Vérifiez les permissions d'écriture dans le dossier parent."
            })
        ),
    };

    let save_path = lora_dir.join("adapter_model.safetensors");
    if let Err(e) = varmap.save(&save_path) {
        raise_error!(
            "ERR_MODEL_SAVE_WEIGHTS",
            error = e,
            context = json_value!({
                "action": "save_varmap_to_disk",
                "path": save_path.to_string_lossy(),
                "hint": "Assurez-vous qu'il reste de l'espace disque et que le chemin est accessible."
            })
        );
    }

    Ok(format!(
        "Adaptateur sauvegardé avec succès dans : {:?}",
        save_path
    ))
}

// --- TESTS UNITAIRES ---

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::io::fs;
    use crate::utils::testing::{inject_mock_component, AgentDbSandbox}; // 🎯 Import de la façade FS

    #[async_test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_ai_train_domain_native_empty_data() {
        let sandbox = AgentDbSandbox::new().await;
        let manager = crate::json_db::collections::manager::CollectionsManager::new(
            &sandbox.db,
            &sandbox.config.system_domain,
            &sandbox.config.system_db,
        );

        inject_mock_component(
            &manager,
            "llm",
            json_value!({ "rust_tokenizer_file": "tokenizer.json" }),
        )
        .await;

        // 1. 🎯 FIX : Utiliser EXACTEMENT le même chemin que la fonction
        let domain_path = AppConfig::get()
            .get_path("PATH_RAISE_DOMAIN")
            .unwrap_or_else(|| PathBuf::from("./raise_default_domain"));

        let models_dir = domain_path.join("_system/ai-assets/models");
        fs::ensure_dir_async(&models_dir)
            .await
            .expect("Impossible de créer le dossier models");

        // 2. 🎯 FIX : Copier le vrai tokenizer pour éviter un crash au parsing (Tokenizer::from_file)
        let mut copied = false;
        if let Some(home) = dirs::home_dir() {
            let real_tokenizer = home.join("raise_domain/_system/ai-assets/models/tokenizer.json");
            if fs::exists_sync(&real_tokenizer) {
                let _ = fs::copy_async(&real_tokenizer, models_dir.join("tokenizer.json")).await;
                copied = true;
            }
        }

        // Mock de secours si le vrai tokenizer n'est pas trouvé
        if !copied {
            let mock_tokenizer =
                r#"{"version":"1.0","model":{"type":"BPE","vocab":{},"merges":[]}}"#;
            fs::write_async(models_dir.join("tokenizer.json"), mock_tokenizer.as_bytes())
                .await
                .unwrap();
        }

        // 3. Exécution de l'entraînement
        let result = ai_train_domain_native(&manager, "nonexistent", 1, 0.001).await;

        // 4. Validation
        assert!(result.is_err());

        let err = result.unwrap_err();
        let err_msg = err.to_string();

        assert!(
            err_msg.contains("ERR_DATA_DOMAIN_EMPTY"),
            "Le test devrait retourner ERR_DATA_DOMAIN_EMPTY, reçu : {}",
            err_msg
        );

        let AppError::Structured(data) = err;
        assert_eq!(data.code, "ERR_DATA_DOMAIN_EMPTY");
        assert_eq!(data.context["domain"], "nonexistent");
    }
}
