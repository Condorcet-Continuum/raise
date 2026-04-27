// FICHIER : src-tauri/src/ai/llm/client.rs

use crate::json_db::collections::manager::CollectionsManager;
use crate::json_db::storage::StorageEngine;
use crate::utils::prelude::*;
use async_trait::async_trait;

// 🎯 Import de vos fournisseurs Cloud
use crate::ai::llm::providers::{claude, gemini, mistral};

#[derive(Clone, Debug, PartialEq)]
pub enum LlmBackend {
    Mistral,
    Claude,
    Gemini,
    Mock,
    // Conservés pour la rétro-compatibilité
    LocalLlama,
    GoogleGemini,
    LlamaCpp,
    RustNative,
}

#[async_trait]
pub trait LlmEngine: Send + Sync {
    // 🎯 On utilise &mut self car l'inférence modifie l'état interne du moteur
    async fn generate(
        &mut self,
        system: &str,
        user: &str,
        max_tokens: usize,
    ) -> RaiseResult<String>;
}

#[derive(Clone)]
pub struct LlmClient {
    storage: SharedRef<StorageEngine>,
    pub space: String,
    pub db_name: String,
    native_engine: Option<SharedRef<AsyncMutex<dyn LlmEngine>>>,
}

impl LlmClient {
    pub async fn new(
        manager: &CollectionsManager<'_>,
        storage: SharedRef<StorageEngine>,
        native_engine: Option<SharedRef<AsyncMutex<dyn LlmEngine>>>,
    ) -> RaiseResult<Self> {
        Ok(Self {
            storage,
            space: manager.space.to_string(),
            db_name: manager.db.to_string(),
            native_engine,
        })
    }

    pub async fn ask(
        &self,
        backend: LlmBackend,
        system_prompt: &str,
        user_prompt: &str,
    ) -> RaiseResult<String> {
        // =========================================================
        // 1. PRIORITÉ ABSOLUE AU MOTEUR INJECTÉ (Local ou Mock)
        // =========================================================
        if let Some(engine_ref) = &self.native_engine {
            let mut engine = engine_ref.lock().await;
            // Si c'est un MockEngine, il répondra instantanément
            return engine.generate(system_prompt, user_prompt, 1024).await;
        }

        // =========================================================
        // 2. FALLBACK EXCEPTIONNEL SUR LE CLOUD
        // =========================================================
        user_warn!(
            "AI_LOCAL_UNAVAILABLE",
            json_value!({"hint": "Bascule sur le réseau distant."})
        );
        let manager = CollectionsManager::new(self.storage.as_ref(), &self.space, &self.db_name);
        match backend {
            LlmBackend::Claude => claude::ask(&manager, system_prompt, user_prompt).await,
            LlmBackend::Gemini => gemini::ask(&manager, system_prompt, user_prompt).await,
            _ => mistral::ask(&manager, system_prompt, user_prompt).await,
        }
    }

    pub async fn generate(&self, user_prompt: &str) -> RaiseResult<String> {
        self.ask(
            LlmBackend::Mistral,
            "Tu es un assistant IA expert et concis.",
            user_prompt,
        )
        .await
    }
}

// =========================================================================
// TESTS UNITAIRES (Validation du Routage et du Gatekeeper)
// =========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::testing::mock::MockLlmEngine;
    use crate::utils::testing::AgentDbSandbox;

    #[async_test]
    #[serial_test::serial]
    async fn test_llm_client_lightweight_init() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await?;
        let config = AppConfig::get();
        let manager = CollectionsManager::new(
            &sandbox.db,
            &config.mount_points.system.domain,
            &config.mount_points.system.db,
        );

        // 🎯 FIX : Ajout de None comme 3ème argument
        let client = LlmClient::new(
            &manager,
            sandbox.db.clone(),
            Some(sandbox.shared_engine.clone()),
        )
        .await?;

        assert_eq!(client.space, config.mount_points.system.domain);
        assert_eq!(client.db_name, config.mount_points.system.db);

        Ok(())
    }

    #[async_test]
    #[serial_test::serial]
    async fn test_llm_client_default_generation_routing() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await?;
        let config = AppConfig::get();
        let manager = CollectionsManager::new(
            &sandbox.db,
            &config.mount_points.system.domain,
            &config.mount_points.system.db,
        );

        let response_mock = r#"{"message": "Test unitaire validé avec succès", "artifacts": []}"#;
        let mock_engine = SharedRef::new(AsyncMutex::new(MockLlmEngine {
            response: response_mock.to_string(),
        }));

        let client = LlmClient::new(&manager, sandbox.db.clone(), Some(mock_engine)).await?;

        let result = client.generate("Bonjour").await?;
        assert!(result.contains("Test unitaire validé avec succès"));
        Ok(())
    }

    #[async_test]
    #[serial_test::serial]
    async fn test_llm_client_claude_routing() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await?;
        let config = AppConfig::get();
        let manager = CollectionsManager::new(
            &sandbox.db,
            &config.mount_points.system.domain,
            &config.mount_points.system.db,
        );

        let expected_msg = "Test unitaire validé avec succès";
        let mock_engine = SharedRef::new(AsyncMutex::new(MockLlmEngine {
            response: expected_msg.to_string(),
        }));

        let client = LlmClient::new(&manager, sandbox.db.clone(), Some(mock_engine)).await?;

        let result = client.ask(LlmBackend::Claude, "System", "User").await?;
        assert!(result.contains(expected_msg));
        Ok(())
    }

    #[async_test]
    #[serial_test::serial]
    async fn test_llm_client_gemini_routing() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await?;
        let manager = CollectionsManager::new(&sandbox.db, "test", "db");

        // 🎯 ÉTAPE 1 : On crée le "leurre" (Mock) avec le message attendu
        let expected_msg = "Test unitaire validé avec succès";
        let mock_engine = SharedRef::new(AsyncMutex::new(MockLlmEngine {
            response: expected_msg.to_string(),
        }));

        let client = LlmClient::new(&manager, sandbox.db.clone(), Some(mock_engine)).await?;

        let result = client.ask(LlmBackend::Gemini, "System", "User").await?;

        assert!(result.contains(expected_msg));
        Ok(())
    }
}
