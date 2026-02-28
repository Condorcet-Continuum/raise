use crate::utils::prelude::*;

use candle_core::{DType, Device, Tensor};
use candle_nn::VarBuilder;
use candle_transformers::models::bert::{BertModel, Config};
use tokenizers::Tokenizer;

pub struct CandleEngine {
    model: BertModel,
    tokenizer: Tokenizer,
    device: Device,
}

impl CandleEngine {
    pub async fn new(
        manager: &crate::json_db::collections::manager::CollectionsManager<'_>,
    ) -> RaiseResult<Self> {
        // 1. DÉTECTION DYNAMIQUE DU MATÉRIEL (GPU > CPU)
        let device = Device::new_metal(0) // Apple Silicon (M1/M2/M3)
            .or_else(|_| Device::new_cuda(0)) // Nvidia CUDA
            .unwrap_or(Device::Cpu); // Fallback CPU standard

        println!("🕯️ [Candle NLP] Moteur initialisé sur : {:?}", device);

        // 2. RÉCUPÉRATION DE LA CONFIGURATION DEPUIS LA DB
        let settings =
            crate::utils::config::AppConfig::get_component_settings(manager, "nlp").await?;

        // Extraction des noms de fichiers (avec fallbacks)
        let model_dir = settings
            .get("model_name")
            .and_then(|v| v.as_str())
            .unwrap_or("minilm");
        let config_filename = settings
            .get("rust_config_file")
            .and_then(|v| v.as_str())
            .unwrap_or("config.json");
        let tokenizer_filename = settings
            .get("rust_tokenizer_file")
            .and_then(|v| v.as_str())
            .unwrap_or("tokenizer.json");
        let weights_filename = settings
            .get("rust_safetensors_file")
            .and_then(|v| v.as_str())
            .unwrap_or("model.safetensors");

        // 3. CONSTRUCTION DES CHEMINS LOCAUX ABSOLUS
        let Some(home) = dirs::home_dir() else {
            raise_error!(
                "ERR_OS_HOME_NOT_FOUND",
                error = "Impossible de localiser le répertoire personnel de l'utilisateur (home).",
                context = json!({ "method": "dirs::home_dir" })
            );
        };

        // On cible dynamiquement le dossier du modèle (ex: embeddings/minilm)
        let base_path = home.join(format!(
            "raise_domain/_system/ai-assets/embeddings/{}",
            model_dir
        ));

        let config_path = base_path.join(config_filename);
        let tokenizer_path = base_path.join(tokenizer_filename);
        let weights_path = base_path.join(weights_filename);

        // 3. Vérification de sécurité stricte
        if !weights_path.exists() || !config_path.exists() || !tokenizer_path.exists() {
            raise_error!(
                "ERR_AI_EMBEDDING_ASSETS_MISSING",
                error = format!("Fichiers d'embeddings introuvables dans : {:?}", base_path),
                context = json!({
                    "base_path": base_path.to_string_lossy(),
                    "missing_files": {
                        "weights": !weights_path.exists(),
                        "config": !config_path.exists(),
                        "tokenizer": !tokenizer_path.exists()
                    }
                })
            );
        }

        // 4. Chargement de la configuration
        let config_str = match std::fs::read_to_string(&config_path) {
            Ok(content) => content,
            Err(e) => raise_error!(
                "ERR_CONFIG_READ",
                error = e,
                context = json!({
                    "action": "read_config_file",
                    // Info CRITIQUE : on logue le chemin exact qui a causé l'échec !
                    "path": config_path.to_string_lossy()
                })
            ),
        };

        // Utilisation de serde_json (ou data::parse) pour lire la config Bert
        let config: Config = match serde_json::from_str(&config_str) {
            Ok(c) => c,
            Err(e) => raise_error!(
                "ERR_CONFIG_PARSE",
                error = e,
                context = json!({
                    "action": "parse_config_json",
                    // Info Magique : On capture les 100 premiers caractères du fichier pour voir si
                    // le contenu est complètement corrompu ou vide, sans inonder les logs !
                    "config_preview": config_str.chars().take(100).collect::<String>()
                })
            ),
        };

        // 5. Chargement du Tokenizer
        let tokenizer = match Tokenizer::from_file(&tokenizer_path) {
            Ok(t) => t,
            Err(e) => raise_error!(
                "ERR_TOKENIZER_LOAD",
                error = e,
                context = json!({
                    "action": "load_tokenizer_file",
                    // Info Vitale : On enregistre le chemin exact où le moteur IA cherchait le fichier
                    "path": tokenizer_path.to_string_lossy()
                })
            ),
        };

        // 6. Chargement des poids (Safetensors)
        let vb = unsafe {
            match VarBuilder::from_mmaped_safetensors(&[&weights_path], DType::F32, &device) {
                Ok(builder) => builder,
                Err(e) => {
                    // Pas de 'return' devant la macro, elle s'en charge.
                    raise_error!(
                        "ERR_AI_WEIGHTS_LOAD_FAILED",
                        error = e,
                        context = json!({
                            "action": "mmap_safetensors",
                            "path": weights_path.to_string_lossy(),
                            "device": format!("{:?}", device),
                            "hint": "Échec du chargement des poids du modèle. Vérifiez que le fichier n'est pas utilisé par un autre processus ou qu'il n'est pas corrompu."
                        })
                    )
                }
            }
        };

        // 7. Initialisation du modèle Bert
        let model = match BertModel::load(vb, &config) {
            Ok(m) => m,
            Err(e) => {
                raise_error!(
                    "ERR_AI_MODEL_INSTANTIATION_FAILED",
                    error = e,
                    context = json!({
                        "action": "load_bert_model",
                        "model_type": "BERT",
                        // On utilise format! pour convertir la config en String
                        "config_debug": format!("{:?}", config),
                        "hint": "Incohérence entre la configuration et les poids du modèle."
                    })
                )
            }
        };

        Ok(Self {
            model,
            tokenizer,
            device,
        })
    }

    fn forward_one(&self, text: &str) -> RaiseResult<Vec<f32>> {
        // 1. Tokenisation
        let tokens = match self.tokenizer.encode(text, true) {
            Ok(t) => t,
            Err(e) => raise_error!(
                "ERR_NLP_TOKENIZATION_FAILED",
                error = e,
                context = json!({ "text_preview": text.chars().take(30).collect::<String>() })
            ),
        };

        // 2. Préparation des Tenseurs
        let token_ids = match Tensor::new(tokens.get_ids(), &self.device) {
            Ok(t) => match t.unsqueeze(0) {
                Ok(u) => u,
                Err(e) => raise_error!("ERR_NLP_TENSOR_SHAPE", error = e),
            },
            Err(e) => raise_error!("ERR_NLP_TENSOR_CREATION", error = e),
        };

        let token_type_ids = match token_ids.zeros_like() {
            Ok(z) => z,
            Err(e) => raise_error!("ERR_NLP_TENSOR_ZEROS", error = e),
        };

        // 3. Inférence BERT
        let embeddings = match self.model.forward(&token_ids, &token_type_ids, None) {
            Ok(emb) => emb,
            Err(e) => raise_error!(
                "ERR_NLP_FORWARD_PASS_FAILED",
                error = e,
                context = json!({ "token_count": tokens.get_ids().len() })
            ),
        };

        // 4. Pooling et Calculs de dimensions
        let (_n_sentence, n_tokens, _hidden_size) = match embeddings.dims3() {
            Ok(d) => d,
            Err(e) => raise_error!("ERR_NLP_DIM_MISMATCH", error = e),
        };

        let sum_embeddings = match embeddings.sum(1) {
            Ok(s) => s,
            Err(e) => raise_error!("ERR_NLP_SUM_FAILED", error = e),
        };

        let pooled = match sum_embeddings / (n_tokens as f64) {
            Ok(p) => p,
            Err(e) => raise_error!("ERR_NLP_POOLING_DIV_FAILED", error = e),
        };

        // 5. Normalisation et Conversion finale
        let normalized = normalize_l2(&pooled)?;

        let vec = match normalized.squeeze(0) {
            Ok(s) => match s.to_vec1::<f32>() {
                Ok(v) => v,
                Err(e) => raise_error!("ERR_NLP_VEC_CONVERSION", error = e),
            },
            Err(e) => raise_error!("ERR_NLP_SQUEEZE_FAILED", error = e),
        };

        Ok(vec)
    }

    pub fn embed_batch(&mut self, texts: Vec<String>) -> RaiseResult<Vec<Vec<f32>>> {
        // Note: Pour une optimisation future, on pourrait tokeniser tout le batch
        // et faire un seul appel forward(), mais cela demande de gérer le Padding manuellement.
        // Avec le GPU activé, cette boucle sera déjà très rapide pour des petits lots.
        let mut results = Vec::new();
        for text in texts {
            results.push(self.forward_one(&text)?);
        }
        Ok(results)
    }

    pub fn embed_query(&mut self, text: &str) -> RaiseResult<Vec<f32>> {
        self.forward_one(text)
    }
}

fn normalize_l2(v: &Tensor) -> RaiseResult<Tensor> {
    // 1. Calcul de la somme des carrés (Sum of Squares)
    let sum_sq = match v.sqr() {
        Ok(s) => match s.sum_keepdim(1) {
            Ok(sum) => sum,
            Err(e) => raise_error!("ERR_NLP_NORM_SUM_FAILED", error = e),
        },
        Err(e) => raise_error!("ERR_NLP_NORM_SQR_FAILED", error = e),
    };

    // 2. Calcul de la racine carrée (Norme)
    let norm = match sum_sq.sqrt() {
        Ok(n) => n,
        Err(e) => raise_error!(
            "ERR_NLP_NORM_SQRT_FAILED",
            error = e,
            context = json!({ "hint": "Vérifiez si le vecteur d'entrée contient des valeurs négatives invalides avant sqrt." })
        ),
    };

    // 3. Division par diffusion (Broadcasting)
    match v.broadcast_div(&norm) {
        Ok(normalized) => Ok(normalized),
        Err(e) => raise_error!(
            "ERR_NLP_NORM_DIV_FAILED",
            error = e,
            context = json!({
                "v_shape": format!("{:?}", v.shape()),
                "norm_shape": format!("{:?}", norm.shape())
            })
        ),
    }
}

// --- TESTS UNITAIRES ---
#[cfg(test)]
mod tests {
    use super::*;

    async fn setup_test_db() -> crate::json_db::storage::StorageEngine {
        crate::utils::config::test_mocks::inject_mock_config();
        let config = crate::utils::config::AppConfig::get();
        let storage_cfg = crate::json_db::storage::JsonDbConfig::new(
            config.get_path("PATH_RAISE_DOMAIN").unwrap(),
        );
        crate::json_db::storage::StorageEngine::new(storage_cfg)
    }

    #[tokio::test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_candle_mini_lm_loading() {
        let storage = setup_test_db().await;
        let config = crate::utils::config::AppConfig::get();
        let manager = crate::json_db::collections::manager::CollectionsManager::new(
            &storage,
            &config.system_domain,
            &config.system_db,
        );
        manager.init_db().await.unwrap();

        crate::utils::config::test_mocks::inject_mock_component(
            &manager,
            "nlp",
            crate::utils::json::json!({
                "model_name": "minilm",
                "rust_config_file": "config.json",
                "rust_tokenizer_file": "tokenizer.json",
                "rust_safetensors_file": "model.safetensors"
            }),
        )
        .await;

        let engine = CandleEngine::new(&manager).await;
        assert!(
            engine.is_ok(),
            "Le modèle MiniLM doit se charger correctement via HF Hub"
        );
    }

    #[tokio::test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_candle_dimensions() {
        let storage = setup_test_db().await;
        let config = crate::utils::config::AppConfig::get();
        let manager = crate::json_db::collections::manager::CollectionsManager::new(
            &storage,
            &config.system_domain,
            &config.system_db,
        );
        manager.init_db().await.unwrap();

        crate::utils::config::test_mocks::inject_mock_component(
            &manager,
            "nlp",
            crate::utils::json::json!({
                "model_name": "minilm",
                "rust_config_file": "config.json",
                "rust_tokenizer_file": "tokenizer.json",
                "rust_safetensors_file": "model.safetensors"
            }),
        )
        .await;

        let mut engine = CandleEngine::new(&manager).await.expect("Init failed");
        let vec = engine.embed_query("Test dimensions").expect("Embed failed");

        assert_eq!(vec.len(), 384);
    }

    #[tokio::test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_candle_normalization() {
        let storage = setup_test_db().await;
        let config = crate::utils::config::AppConfig::get();
        let manager = crate::json_db::collections::manager::CollectionsManager::new(
            &storage,
            &config.system_domain,
            &config.system_db,
        );
        manager.init_db().await.unwrap();

        crate::utils::config::test_mocks::inject_mock_component(
            &manager,
            "nlp",
            crate::utils::json::json!({
                "model_name": "minilm",
                "rust_config_file": "config.json",
                "rust_tokenizer_file": "tokenizer.json",
                "rust_safetensors_file": "model.safetensors"
            }),
        )
        .await;

        let mut engine = CandleEngine::new(&manager).await.expect("Init failed");
        let vec = engine.embed_query("Mathematiques").expect("Embed failed");

        let sum_sq: f32 = vec.iter().map(|x| x * x).sum();
        let norm = sum_sq.sqrt();

        assert!(
            (norm - 1.0).abs() < 1e-4,
            "Le vecteur doit être normalisé (Norme proche de 1.0), actuel: {}",
            norm
        );
    }
}
