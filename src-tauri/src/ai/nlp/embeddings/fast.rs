// FICHIER : src-tauri/src/ai/nlp/embeddings/fast.rs

use crate::json_db::collections::manager::CollectionsManager;
use crate::utils::prelude::*; // 🎯 Façade Unique

pub struct FastEmbedEngine {
    model: LightweightTextEmbedding,
}

impl FastEmbedEngine {
    /// Initialise le moteur FastEmbed en respectant l'isolation stricte du domaine RAISE.
    pub async fn new(manager: &CollectionsManager<'_>) -> RaiseResult<Self> {
        // 1. Appel du Gatekeeper (Tolérance aux pannes pour le moteur par défaut)
        let settings =
            match AppConfig::get_runtime_settings(manager, "ref:components:handle:ai_nlp").await {
                Ok(s) => s,
                Err(_) => {
                    // FastEmbed est le moteur léger de secours (Fallback absolu).
                    json_value!({})
                }
            };

        // 2. Extraction de la valeur (avec fallback par défaut)
        let model_name_str = settings
            .get("fastembed_model")
            .and_then(|v| v.as_str())
            .unwrap_or("BGESmallENV15");

        // 3. Déduction dynamique du modèle ONNX
        let embed_model = match model_name_str {
            "AllMiniLML6V2" => LightweightEmbeddingModel::AllMiniLML6V2,
            _ => LightweightEmbeddingModel::BGESmallENV15,
        };

        // 4. 🎯 Rapatriement du cache FastEmbed dans le domaine RAISE (Zéro Dette)
        let config = AppConfig::get();
        let raise_domain_path = config
            .get_path("PATH_RAISE_DOMAIN")
            .unwrap_or_else(|| PathBuf::from("./raise_domain"));

        let isolated_cache_dir = raise_domain_path
            .join("_system")
            .join("ai-assets")
            .join("embeddings")
            .join("fastembed");

        // On s'assure que le dossier existe via la façade I/O
        if let Err(e) = fs::ensure_dir_sync(&isolated_cache_dir) {
            raise_error!("ERR_AI_FASTEMBED_CACHE", error = e.to_string());
        }

        // 5. Paramétrage avec l'isolation du cache
        let options = LightweightInitOptions::new(embed_model)
            .with_show_download_progress(true)
            .with_cache_dir(isolated_cache_dir); // 🎯 L'isolation est garantie ici

        // 6. Initialisation sécurisée via Match
        let model = match LightweightTextEmbedding::try_new(options) {
            Ok(m) => m,
            Err(e) => raise_error!(
                "ERR_AI_FASTEMBED_INIT",
                error = e.to_string(),
                context = json_value!({
                    "provider": "FastEmbed",
                    "model": model_name_str
                })
            ),
        };

        user_info!(
            "MSG_NLP_FASTEMBED_READY",
            json_value!({ "model": model_name_str, "status": "initialized" })
        );

        Ok(Self { model })
    }

    /// Vectorise un lot de textes (Batch Inference) pour optimiser le débit.
    pub fn embed_batch(&mut self, texts: Vec<String>) -> RaiseResult<Vec<Vec<f32>>> {
        let batch_size = texts.len();
        if batch_size == 0 {
            return Ok(Vec::new());
        }

        match self.model.embed(texts, None) {
            Ok(embeddings) => Ok(embeddings),
            Err(e) => raise_error!(
                "ERR_AI_EMBEDDINGS_BATCH_FAILED",
                error = e.to_string(),
                context = json_value!({
                    "batch_size": batch_size,
                    "provider": "FastEmbed"
                })
            ),
        }
    }

    /// Vectorise une requête unique.
    pub fn embed_query(&mut self, text: &str) -> RaiseResult<Vec<f32>> {
        let embeddings = match self.model.embed(vec![text.to_string()], None) {
            Ok(e) => e,
            Err(e) => raise_error!(
                "ERR_AI_EMBEDDING_QUERY_FAILED",
                error = e.to_string(),
                context = json_value!({ "text_len": text.len(), "provider": "FastEmbed" })
            ),
        };

        // 🎯 Rigueur : Extraction sécurisée du premier vecteur
        let mut iter = embeddings.into_iter();
        match iter.next() {
            Some(vector) => Ok(vector),
            None => raise_error!(
                "ERR_AI_EMBEDDING_EMPTY_RESULT",
                error = "Le moteur n'a retourné aucun vecteur.",
                context = json_value!({ "text_len": text.len() })
            ),
        }
    }
}

// =========================================================================
// TESTS UNITAIRES (Rigueur Façade & Résilience)
// =========================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::testing::AgentDbSandbox;

    /// Test existant : Inférence simple
    #[async_test]
    #[serial_test::serial]
    async fn test_fast_embed_single() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await?;
        let config = AppConfig::get();

        // 🎯 FIX MOUNT POINTS : Utilisation du domaine système configuré
        let manager = CollectionsManager::new(
            &sandbox.db,
            &config.mount_points.system.domain,
            &config.mount_points.system.db,
        );

        let mut engine = FastEmbedEngine::new(&manager).await?;
        let vec = engine.embed_query("Ceci est un test de la façade RAISE")?;

        assert_eq!(
            vec.len(),
            384,
            "BGE-Small-EN-V1.5 doit retourner 384 dimensions"
        );
        assert!(vec.iter().any(|&x| x != 0.0));
        Ok(())
    }

    /// Test existant : Inférence par lot
    #[async_test]
    #[serial_test::serial]
    async fn test_fast_embed_batch() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await?;
        let config = AppConfig::get();
        let manager = CollectionsManager::new(
            &sandbox.db,
            &config.mount_points.system.domain,
            &config.mount_points.system.db,
        );

        let mut engine = FastEmbedEngine::new(&manager).await?;
        let inputs = vec![
            "Phrase 1".to_string(),
            "Phrase 2".to_string(),
            "Phrase 3".to_string(),
        ];

        let batch_res = engine.embed_batch(inputs)?;
        assert_eq!(batch_res.len(), 3);
        assert_eq!(batch_res[0].len(), 384);
        Ok(())
    }

    /// Résilience face à un domaine Système vide (Default Fallback)
    #[async_test]
    #[serial_test::serial]
    async fn test_fast_embed_resilience_empty_config() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await?;
        // Manager pointant sur un domaine vierge
        let manager = CollectionsManager::new(&sandbox.db, "void", "void");

        // L'initialisation doit réussir en utilisant les valeurs par défaut (BGESmallENV15)
        let engine_res = FastEmbedEngine::new(&manager).await;
        assert!(
            engine_res.is_ok(),
            "Le moteur doit fallback sur les paramètres par défaut"
        );
        Ok(())
    }

    /// 🎯 NOUVEAU TEST : Inférence sur chaîne vide
    #[async_test]
    #[serial_test::serial]
    async fn test_fast_embed_empty_string() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await?;
        let config = AppConfig::get();
        let manager = CollectionsManager::new(
            &sandbox.db,
            &config.mount_points.system.domain,
            &config.mount_points.system.db,
        );

        let mut engine = FastEmbedEngine::new(&manager).await?;
        let vec = engine.embed_query("");

        assert!(
            vec.is_ok(),
            "Le moteur ONNX doit gérer les chaînes vides sans paniquer"
        );
        Ok(())
    }
}
