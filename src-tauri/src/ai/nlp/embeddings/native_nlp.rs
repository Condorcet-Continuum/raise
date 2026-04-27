// FICHIER : src-tauri/src/ai/nlp/embeddings/native_nlp.rs

use crate::kernel::assets::AssetResolver;
use crate::utils::prelude::*; // 🎯 Façade Unique

pub struct NativeNlpEngine {
    model: NeuralBertModel,
    tokenizer: TextTokenizer,
    device: ComputeHardware,
}

impl NativeNlpEngine {
    /// Initialise le moteur d'embeddings BERT en forçant l'utilisation du CPU.
    pub async fn new(
        manager: &crate::json_db::collections::manager::CollectionsManager<'_>,
    ) -> RaiseResult<Self> {
        // 🎯 CORRECTION CPU ICI : On libère la VRAM à 100% pour le LLM natif.
        // Les embeddings tourneront de manière très véloce sur le processeur (RAM classique).
        let device = ComputeHardware::Cpu;

        user_info!(
            "MSG_NLP_NATIVE_INIT",
            json_value!({ "device": format!("{:?}", device), "backend": "BERT (CPU Forced)" })
        );

        // 1. Appel du Gatekeeper (Routage + Vérification d'Activation + Standard Raise)
        let settings = match AppConfig::get_runtime_settings(
            manager,
            "ref:components:handle:ai_nlp",
        )
        .await
        {
            Ok(s) => s,
            Err(e) => raise_error!(
                "ERR_NLP_NATIVE_CONFIG",
                error = e.to_string(),
                context = json_value!({"hint": "Vérifiez que ai_nlp est actif dans la configuration système."})
            ),
        };

        // 2. Extraction des valeurs avec fallback
        let model_dir = match settings.get("model_name").and_then(|v| v.as_str()) {
            Some(v) => v,
            None => raise_error!(
                "ERR_NLP_MISSING_VAR",
                error = "La variable 'model_name' est introuvable.",
                context = json_value!({"component": "ai_nlp"})
            ),
        };
        let config_filename = match settings.get("rust_config_file").and_then(|v| v.as_str()) {
            Some(v) => v,
            None => raise_error!(
                "ERR_NLP_MISSING_VAR",
                error = "La variable 'rust_config_file' est introuvable.",
                context = json_value!({"component": "ai_nlp"})
            ),
        };
        let tokenizer_filename = match settings.get("rust_tokenizer_file").and_then(|v| v.as_str())
        {
            Some(v) => v,
            None => raise_error!(
                "ERR_NLP_MISSING_VAR",
                error = "La variable 'rust_tokenizer_file' est introuvable.",
                context = json_value!({"component": "ai_nlp"})
            ),
        };
        let weights_filename = match settings
            .get("rust_safetensors_file")
            .and_then(|v| v.as_str())
        {
            Some(v) => v,
            None => raise_error!(
                "ERR_NLP_MISSING_VAR",
                error = "La variable 'rust_safetensors_file' est introuvable.",
                context = json_value!({"component": "ai_nlp"})
            ),
        };

        // 3. Résolution dynamique de la racine via AppConfig
        let config_global = AppConfig::get();
        let primary_base_path = config_global
            .resolve_asset_path(
                config_global
                    .system_assets
                    .ai_assets_paths
                    .as_ref()
                    .and_then(|p| p.embeddings.as_ref()),
                "ai-assets/embeddings",
            )?
            .join(model_dir);

        let category = format!("ai-assets/embeddings/{}", model_dir);

        // 4. 🎯 Résolution factorisée via AssetResolver (Zéro Dette)
        let resolve_or_fail = |filename: &str| -> RaiseResult<PathBuf> {
            match AssetResolver::resolve_ai_file_sync(&primary_base_path, &category, filename) {
                Some(p) => Ok(p),
                None => raise_error!(
                    "ERR_AI_EMBEDDING_ASSETS_MISSING",
                    error = format!("Fichier NLP introuvable : {}", filename),
                    context = AssetResolver::missing_file_context(
                        &primary_base_path,
                        &category,
                        filename
                    )
                ),
            }
        };

        let config_path = resolve_or_fail(config_filename)?;
        let tokenizer_path = resolve_or_fail(tokenizer_filename)?;
        let weights_path = resolve_or_fail(weights_filename)?;

        // 5. Chargement sécurisé de la configuration BERT
        let config_str = match fs::read_to_string_sync(&config_path) {
            Ok(content) => content,
            Err(e) => raise_error!("ERR_NLP_CONFIG_READ", error = e.to_string()),
        };

        let bert_config: NeuralBertConfig = match json::deserialize_from_str(&config_str) {
            Ok(c) => c,
            Err(e) => raise_error!("ERR_NLP_CONFIG_PARSE", error = e.to_string()),
        };

        // 6. Chargement du TextTokenizer
        let tokenizer = match TextTokenizer::from_file(&tokenizer_path) {
            Ok(t) => t,
            Err(e) => raise_error!("ERR_NLP_TOKENIZER_LOAD", error = e.to_string()),
        };

        // 7. Chargement des poids via Memory Mapping (Sur CPU !)
        let vb = unsafe {
            match NeuralWeightsBuilder::from_mmaped_safetensors(
                &[&weights_path],
                ComputeType::F32,
                &device, // 🎯 Utilise désormais ComputeHardware::Cpu
            ) {
                Ok(builder) => builder,
                Err(e) => raise_error!("ERR_NLP_WEIGHTS_LOAD", error = e.to_string()),
            }
        };

        let model = match NeuralBertModel::load(vb, &bert_config) {
            Ok(m) => m,
            Err(e) => raise_error!("ERR_NLP_MODEL_INSTANTIATION", error = e.to_string()),
        };

        Ok(Self {
            model,
            tokenizer,
            device,
        })
    }

    /// Vectorise un lot de textes (Batch Inference)
    pub fn embed_batch(&mut self, texts: Vec<String>) -> RaiseResult<Vec<Vec<f32>>> {
        let batch_size = texts.len();
        if batch_size == 0 {
            return Ok(Vec::new());
        }

        let encodings = match self.tokenizer.encode_batch(texts, true) {
            Ok(e) => e,
            Err(e) => raise_error!("ERR_NLP_BATCH_TOKENIZATION", error = e.to_string()),
        };

        let max_len = encodings
            .iter()
            .map(|e| e.get_ids().len())
            .max()
            .unwrap_or(0);
        let mut batch_ids = Vec::with_capacity(batch_size * max_len);
        let batch_type_ids = vec![0u32; batch_size * max_len];

        for enc in &encodings {
            let ids = enc.get_ids();
            batch_ids.extend_from_slice(ids);
            batch_ids.resize(batch_ids.len() + (max_len - ids.len()), 0);
        }

        let token_ids = match NeuralTensor::from_vec(batch_ids, (batch_size, max_len), &self.device)
        {
            Ok(t) => t,
            Err(e) => raise_error!("ERR_NLP_TENSOR_IDS", error = e.to_string()),
        };

        let token_type_ids =
            match NeuralTensor::from_vec(batch_type_ids, (batch_size, max_len), &self.device) {
                Ok(t) => t,
                Err(e) => raise_error!("ERR_NLP_TENSOR_TYPES", error = e.to_string()),
            };

        let embeddings = match self.model.forward(&token_ids, &token_type_ids, None) {
            Ok(emb) => emb,
            Err(e) => raise_error!("ERR_NLP_FORWARD_PASS", error = e.to_string()),
        };

        // Pooling sémantique (Moyenne)
        let pooled = match embeddings.sum(1)? / (max_len as f64) {
            Ok(p) => p,
            Err(e) => raise_error!("ERR_NLP_POOLING", error = e.to_string()),
        };

        let normalized = normalize_l2(&pooled)?;

        match normalized.to_vec2::<f32>() {
            Ok(matrix) => Ok(matrix),
            Err(e) => raise_error!("ERR_NLP_VEC_CONVERSION", error = e.to_string()),
        }
    }

    pub fn embed_query(&mut self, text: &str) -> RaiseResult<Vec<f32>> {
        let mut batch_res = self.embed_batch(vec![text.to_string()])?;
        batch_res
            .pop()
            .ok_or_else(|| build_error!("ERR_NLP_EMPTY_RESULT", error = "Inférence nulle"))
    }
}

fn normalize_l2(v: &NeuralTensor) -> RaiseResult<NeuralTensor> {
    let sum_sq = v.sqr()?.sum_keepdim(1)?;
    let epsilon = NeuralTensor::new(&[1e-8f32], v.device())?;
    let norm = sum_sq.broadcast_add(&epsilon)?.sqrt()?;

    match v.broadcast_div(&norm) {
        Ok(normalized) => Ok(normalized),
        Err(e) => raise_error!("ERR_NLP_NORMALIZATION_FAILED", error = e.to_string()),
    }
}

// =========================================================================
// TESTS UNITAIRES
// =========================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use crate::json_db::collections::manager::CollectionsManager;
    use crate::utils::testing::AgentDbSandbox;

    async fn provide_assets_to_sandbox(model_name: &str) {
        let config = AppConfig::get();
        let domain_path = config
            .get_path("PATH_RAISE_DOMAIN")
            .unwrap_or_else(|| PathBuf::from("./raise_default_domain"));

        if let Some(home) = dirs::home_dir() {
            let real_path = home.join(format!(
                "raise_domain/_system/ai-assets/embeddings/{}",
                model_name
            ));

            let target_path = domain_path
                .join(&config.mount_points.system.domain)
                .join(&config.mount_points.system.db)
                .join(format!("ai-assets/embeddings/{}", model_name));

            if fs::exists_async(&real_path).await {
                let _ = fs::ensure_dir_async(&target_path).await;
                let _ = fs::copy_async(
                    real_path.join("config.json"),
                    target_path.join("config.json"),
                )
                .await;
                let _ = fs::copy_async(
                    real_path.join("tokenizer.json"),
                    target_path.join("tokenizer.json"),
                )
                .await;
                let _ = fs::copy_async(
                    real_path.join("model.safetensors"),
                    target_path.join("model.safetensors"),
                )
                .await;
            }
        }
    }

    #[async_test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_native_mini_lm_loading() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await?;
        provide_assets_to_sandbox("minilm").await;

        let manager = CollectionsManager::new(
            &sandbox.db,
            &sandbox.config.mount_points.system.domain,
            &sandbox.config.mount_points.system.db,
        );

        let engine = NativeNlpEngine::new(&manager).await?;
        assert!(engine.tokenizer.get_vocab_size(true) > 0);
        Ok(())
    }

    #[async_test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_native_dimensions_and_norm() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await?;
        provide_assets_to_sandbox("minilm").await;

        let manager = CollectionsManager::new(
            &sandbox.db,
            &sandbox.config.mount_points.system.domain,
            &sandbox.config.mount_points.system.db,
        );

        let mut engine = NativeNlpEngine::new(&manager).await?;
        let vec = engine.embed_query("Test NLP")?;

        assert_eq!(vec.len(), 384);
        let norm: f32 = vec.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 1e-4);
        Ok(())
    }

    #[async_test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_resilience_missing_nlp_assets() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await?;
        let config = AppConfig::get();
        let manager = CollectionsManager::new(
            &sandbox.db,
            &config.mount_points.system.domain,
            &config.mount_points.system.db,
        );

        let mut nlp_doc = manager
            .get_document("service_configs", "cfg_ai_nlp_test")
            .await?
            .expect("La config NLP devrait être présente via AgentDbSandbox");

        nlp_doc["service_settings"]["rust_safetensors_file"] =
            json_value!("this_file_does_not_exist.safetensors");

        let _ = manager
            .delete_document("service_configs", "cfg_ai_nlp_test")
            .await;
        manager.insert_raw("service_configs", &nlp_doc).await?;

        let result = NativeNlpEngine::new(&manager).await;
        match result {
            Err(AppError::Structured(err)) => {
                assert_eq!(err.code, "ERR_AI_EMBEDDING_ASSETS_MISSING");
                Ok(())
            }
            _ => panic!("Le moteur aurait dû lever ERR_AI_EMBEDDING_ASSETS_MISSING"),
        }
    }
}
