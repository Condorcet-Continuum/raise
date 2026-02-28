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
) -> RaiseResult<String> {
    let _device = Device::new_cuda(0).unwrap_or(Device::Cpu);
    // ---------------------------------------------------------
    // 1. RÉCUPÉRATION DU TOKENIZER DEPUIS LA DB
    // ---------------------------------------------------------
    let config_app = AppConfig::get();
    let manager = crate::json_db::collections::manager::CollectionsManager::new(
        storage,
        &config_app.system_domain,
        &config_app.system_db,
    );

    let settings = AppConfig::get_component_settings(&manager, "llm").await?;

    let tokenizer_filename = settings
        .get("rust_tokenizer_file")
        .and_then(|v| v.as_str())
        .unwrap_or("tokenizer.json");

    let Some(home) = dirs::home_dir() else {
        raise_error!(
            "ERR_OS_HOME_NOT_FOUND",
            error = "Impossible de localiser le répertoire personnel de l'utilisateur (home).",
            context = json!({ "method": "dirs::home_dir" })
        );
    };
    // On pointe vers notre dossier de modèles locaux
    let tokenizer_path = home
        .join("raise_domain/_system/ai-assets/models")
        .join(tokenizer_filename);

    // MIGRATION V1.3 : Validation de l'existence du tokenizer avec erreur structurée
    if !tokenizer_path.exists() {
        raise_error!(
            "ERR_AI_TOKENIZER_FILE_NOT_FOUND",
            error = format!("Fichier Tokenizer introuvable : {:?}", tokenizer_path),
            context = json!({ "path": tokenizer_path.to_string_lossy() })
        );
    }

    let tokenizer = match Tokenizer::from_file(&tokenizer_path) {
        Ok(t) => t,
        Err(e) => raise_error!(
            "ERR_AI_TOKENIZER_LOAD",
            error = e,
            context = json!({
                "path": tokenizer_path.to_string_lossy(),
                "action": "load_tokenizer_from_file"
            })
        ),
    };
    println!("✅ Tokenizer '{}' chargé avec succès.", tokenizer_filename);

    // ---------------------------------------------------------
    // 2. EXTRACTION DES DONNÉES
    // ---------------------------------------------------------
    let examples = dataset::extract_domain_data(storage, space, db_name, domain).await?;

    if examples.is_empty() {
        raise_error!(
            "ERR_DATA_DOMAIN_EMPTY",
            error = "EMPTY_COLLECTION", // Étiquette statique pour l'erreur
            context = json!({
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
            error = e, // On préserve l'erreur native de Candle/AdamW
            context = json!({
                "action": "initialize_adamw",
                "learning_rate": lr,
                "variable_count": varmap.all_vars().len(),
                "hint": "Vérifiez que les variables du modèle sont correctement allouées sur le même device."
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
            // A. Formatage du texte au format ChatML (compris par Qwen)
            let prompt = format!(
                "<|im_start|>system\n{}<|im_end|>\n<|im_start|>user\n{}<|im_end|>\n<|im_start|>assistant\n{}<|im_end|>",
                example.instruction, example.input, example.output
            );

            // B. Tokenisation : Texte -> Nombres (IDs)
            let prompt_len = prompt.len();
            let encoding = match tokenizer.encode(prompt, true) {
                Ok(enc) => enc,
                Err(e) => raise_error!(
                    "ERR_AI_TOKENIZATION_FAIL",
                    error = e,
                    context = json!({
                        "prompt_len": prompt_len, // On utilise la variable locale, pas prompt.len()
                        "add_special_tokens": true
                    })
                ),
            };

            let tokens = encoding.get_ids();
            let seq_len = tokens.len(); // La vraie longueur de notre séquence !

            // C. Conversion en Tenseur Candle [1, seq_len] de type U32
            // 1. Création du tenseur de base
            let labels_base = match Tensor::new(tokens, &_device) {
                Ok(t) => t,
                Err(e) => raise_error!(
                    "ERR_TENSOR_CREATE",
                    error = e,
                    context = json!({
                        "action": "create_labels_tensor",
                        "token_count": tokens.len(),
                        "device": format!("{:?}", _device)
                    })
                ),
            };

            // 2. Changement de dimension (Unsqueeze)
            let labels = match labels_base.unsqueeze(0) {
                Ok(t) => t,
                Err(e) => raise_error!(
                    "ERR_TENSOR_SHAPE_UNSQUEEZE",
                    error = e,
                    context = json!({
                        "action": "unsqueeze_labels",
                        "current_shape": format!("{:?}", labels_base.shape()),
                        "dim": 0
                    })
                ),
            };

            // D. Simulation du Forward Pass de Qwen (en attendant le Boss final)
            let vocab_size = 151936; // Qwen 2.5 vocab size
            let dummy_logits = match Tensor::randn(0f32, 1f32, (1, seq_len, vocab_size), &_device) {
                Ok(t) => t,
                Err(e) => raise_error!(
                    "ERR_TENSOR_RANDN_INIT",
                    error = e,
                    context = json!({
                        "action": "create_dummy_logits",
                        "shape": [1, seq_len, vocab_size],
                        "device": format!("{:?}", _device),
                        "hint": "L'échec de randn indique souvent un manque de mémoire VRAM ou une taille de vocabulaire/séquence démesurée."
                    })
                ),
            };

            // E. Calcul de la Loss (Erreur)
            // 1. On prépare les logits (aplatissement)
            let flat_logits = match dummy_logits.flatten_to(1) {
                Ok(t) => t,
                Err(e) => raise_error!(
                    "ERR_MODEL_FLATTEN_LOGITS",
                    error = e,
                    context = json!({ "shape": format!("{:?}", dummy_logits.shape()) })
                ),
            };

            // 2. On prépare les labels (aplatissement)
            let flat_labels = match labels.flatten_to(1) {
                Ok(t) => t,
                Err(e) => raise_error!(
                    "ERR_MODEL_FLATTEN_LABELS",
                    error = e,
                    context = json!({ "shape": format!("{:?}", labels.shape()) })
                ),
            };

            // 3. Calcul de la Cross Entropy
            let loss = match candle_nn::loss::cross_entropy(&flat_logits, &flat_labels) {
                Ok(l) => l,
                Err(e) => raise_error!(
                    "ERR_MODEL_CROSS_ENTROPY",
                    error = e,
                    context = json!({
                        "action": "compute_loss",
                        "logits_shape": format!("{:?}", flat_logits.shape()),
                        "labels_shape": format!("{:?}", flat_labels.shape())
                    })
                ),
            };

            // F. Rétropropagation (Apprentissage)
            match _opt.backward_step(&loss) {
                Ok(_) => (), // L'opération a réussi, on continue
                Err(e) => raise_error!(
                    "ERR_MODEL_BACKPROP",
                    error = e,
                    context = json!({
                        "action": "backward_step",
                        "phase": "model_optimization",
                        // Info utile pour l'IA : on sait exactement quelle étape mathématique a échoué
                        "target": "loss_gradients"
                    })
                ),
            };
            epoch_loss += match loss.to_vec0::<f32>() {
                Ok(val) => val,
                Err(e) => raise_error!(
                    "ERR_MODEL_LOSS_CONVERSION",
                    error = e, // 🚀 Fini le e.to_string() ! On passe l'erreur native.
                    context = json!({
                        "action": "extract_loss_value",
                        "phase": "epoch_accumulation",
                        "expected_type": "f32 scalar",
                        "hint": "Le tenseur de loss ne contient probablement pas une valeur scalaire unique."
                    })
                ),
            };
        }

        let avg_loss = epoch_loss / examples.len() as f32;
        println!("   📉 Loss moyenne: {:.4}", avg_loss);
    }

    // ... (Le code de sauvegarde du dossier LoRA reste inchangé ici) ...
    let Some(home) = dirs::home_dir() else {
        raise_error!(
            "ERR_SYSTEM_HOME_NOT_FOUND",
            error = "OS_ENV_ERROR", // On définit une erreur statique puisque dirs ne renvoie pas d'objet Error
            context = json!({
                "action": "resolve_home_directory",
                "hint": "Vérifiez les variables d'environnement HOME ou USERPROFILE."
            })
        );
    };
    let lora_dir = home
        .join("raise_domain/_system/ai-assets/lora")
        .join(format!("raise-{}-adapter", domain));
    match std::fs::create_dir_all(&lora_dir) {
        Ok(_) => (), // Le dossier existe ou a été créé, tout va bien
        Err(e) => raise_error!(
            "ERR_FS_LORA_DIR_CREATE",
            error = e,
            context = json!({
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
            context = json!({
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
    use crate::json_db::storage::JsonDbConfig;
    use crate::utils::io::tempdir;

    #[tokio::test]
    #[serial_test::serial]
    // On garde l'ignore si CUDA n'est pas là car Device::new_cuda(0) est "hardcoded" dans ta fonction
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_ai_train_domain_native_empty_data() {
        crate::utils::config::test_mocks::inject_mock_config();

        let temp_dir = tempdir().expect("Échec dossier temp");
        // On mocke le home directory pour que le chargement du tokenizer ne panique pas
        // (Note: Assure-tu que tokenizer.json existe dans le dossier mocké ou que le chargement est court-circuité)

        let config = JsonDbConfig::new(temp_dir.path().to_path_buf());
        let storage = StorageEngine::new(config);

        let app_config = AppConfig::get();
        let manager = crate::json_db::collections::manager::CollectionsManager::new(
            &storage,
            &app_config.system_domain,
            &app_config.system_db,
        );
        manager.init_db().await.unwrap();

        crate::utils::config::test_mocks::inject_mock_component(
            &manager,
            "llm",
            crate::utils::json::json!({ "rust_tokenizer_file": "tokenizer.json" }),
        )
        .await;

        let result = ai_train_domain_native(&storage, "space", "db", "nonexistent", 1, 0.001).await;

        // 🎯 VALIDATION DU NOUVEAU STANDARD D'ERREUR
        assert!(result.is_err());

        let err = result.unwrap_err();
        let err_msg = err.to_string();

        // On vérifie la présence du CODE d'erreur structuré
        assert!(
            err_msg.contains("ERR_DATA_DOMAIN_EMPTY"),
            "Le test devrait retourner ERR_DATA_DOMAIN_EMPTY, reçu : {}",
            err_msg
        );

        // Optionnel : On peut même vérifier le contexte injecté !
        let crate::utils::error::AppError::Structured(data) = err;

        assert_eq!(data.code, "ERR_DATA_DOMAIN_EMPTY");
        assert_eq!(data.context["domain"], "nonexistent");
    }
}
