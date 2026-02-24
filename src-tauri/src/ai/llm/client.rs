use crate::ai::llm::candle_engine::CandleLlmEngine;
use crate::json_db::collections::manager::CollectionsManager;
use crate::utils::{prelude::*, Arc, AsyncMutex};

// On garde l'énumération pour la rétrocompatibilité avec tes agents existants,
// mais elle n'a plus d'impact réel sous le capot !
#[derive(Clone, Debug)]
pub enum LlmBackend {
    LocalLlama,
    GoogleGemini,
    LlamaCpp,
    RustNative,
}

#[derive(Clone)]
pub struct LlmClient {
    engine: Arc<AsyncMutex<CandleLlmEngine>>,
}

impl LlmClient {
    pub async fn new(manager: &CollectionsManager<'_>) -> RaiseResult<Self> {
        // Initialisation directe du moteur IA local
        let engine = CandleLlmEngine::new(manager).await?;
        Ok(Self {
            engine: Arc::new(AsyncMutex::new(engine)),
        })
    }

    pub async fn ask(
        &self,
        _backend: LlmBackend, // 🎯 Ignoré : Tout passe désormais en mode 100% hors-ligne !
        system_prompt: &str,
        user_prompt: &str,
    ) -> RaiseResult<String> {
        let mut engine = self.engine.lock().await;
        // On génère 1024 tokens par défaut
        engine.generate(system_prompt, user_prompt, 1024)
    }
}
