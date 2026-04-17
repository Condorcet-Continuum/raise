use crate::ai::llm::native_engine::NativeTensorEngine;
use crate::json_db::collections::manager::CollectionsManager;
use crate::utils::prelude::*;

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
    engine: SharedRef<SyncMutex<NativeTensorEngine>>,
}

impl LlmClient {
    pub async fn new(manager: &CollectionsManager<'_>) -> RaiseResult<Self> {
        // Initialisation directe du moteur IA local
        let engine = NativeTensorEngine::new(manager).await?;
        Ok(Self {
            engine: SharedRef::new(SyncMutex::new(engine)),
        })
    }

    pub async fn ask(
        &self,
        _backend: LlmBackend,
        system_prompt: &str,
        user_prompt: &str,
    ) -> RaiseResult<String> {
        let engine_ref = self.engine.clone();
        let sys_clone = system_prompt.to_string();
        let usr_clone = user_prompt.to_string();

        // Utilisation de la façade OS spécifique (Snake Case)
        // On déporte l'inférence lourde sans polluer le client avec de la plomberie asynchrone.
        // On génère 1024 tokens par défaut
        os::execute_native_inference(move || {
            let mut engine = match engine_ref.lock() {
                Ok(guard) => guard,
                Err(e) => raise_error!(
                    "ERR_AI_MUTEX_POISONED",
                    error = e.to_string(), // PoisonError implémente Display
                    context = json_value!({
                        "action": "lock_llm_engine",
                        "hint": "Le thread LLM a crashé précédemment, rendant le moteur inaccessible."
                    })
                ),
            };
            engine.generate(&sys_clone, &usr_clone, 1024)
        })
        .await
    }
}
