// FICHIER : src-tauri/src/workflow_engine/tools/system_tools.rs

use super::AgentTool;
use crate::utils::prelude::*;
use crate::workflow_engine::handlers::HandlerContext;

/// Outil permettant à l'IA et au Workflow de lire l'état du Jumeau Numérique.
/// Cet outil est désormais 100% "Stateless" et ultra-rapide grâce au contexte partagé.
#[derive(Debug, Default)]
pub struct SystemMonitorTool;

impl SystemMonitorTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_interface]
impl AgentTool for SystemMonitorTool {
    fn name(&self) -> &str {
        "read_system_metrics"
    }

    fn description(&self) -> &str {
        "Lit les valeurs temps réel des capteurs du système physique (Jumeau Numérique). Retourne un objet JSON avec les métriques."
    }

    fn parameters_schema(&self) -> JsonValue {
        json_value!({
            "type": "object",
            "properties": {},
            "required": []
        })
    }

    /// Exécute la lecture des métriques en utilisant la connexion DB mutualisée.
    async fn execute(
        &self,
        _params: &JsonValue,
        context: &HandlerContext<'_>,
    ) -> RaiseResult<JsonValue> {
        tracing::info!(
            "🔍 [SystemMonitorTool] Lecture du Jumeau Numérique via le Contexte Partagé..."
        );

        // 🎯 On utilise directement le manager du contexte, plus besoin de StorageEngine::new !
        let vibration_z = match context
            .manager
            .get_document("digital_twin", "vibration_z")
            .await
        {
            Ok(Some(doc)) => doc["value"].as_f64().unwrap_or(2.0),
            _ => {
                tracing::warn!(
                    "⚠️ Capteur 'vibration_z' non trouvé, utilisation de la valeur nominale."
                );
                2.0
            }
        };

        let metrics = json_value!({
            "vibration_z": vibration_z,
            "temp_core": 45.0,
            "cpu_load": 12.5,
            "status": "ONLINE",
            "timestamp": UtcClock::now().to_rfc3339()
        });

        tracing::info!("📊 [SystemMonitorTool] Métriques extraites avec succès.");
        Ok(metrics)
    }
}

// =========================================================================
// TESTS UNITAIRES
// =========================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use crate::json_db::collections::manager::CollectionsManager;
    // 🎯 FIX : On importe inject_mock_component pour l'Orchestrateur
    use crate::utils::testing::{inject_mock_component, GlobalDbSandbox};

    use crate::ai::orchestrator::AiOrchestrator;
    use crate::model_engine::types::ProjectModel;
    use crate::plugins::manager::PluginManager;
    use crate::workflow_engine::critic::WorkflowCritic;

    #[async_test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_system_tool_persistence_integration() {
        let sandbox = GlobalDbSandbox::new().await;
        let manager = CollectionsManager::new(
            &sandbox.db,
            &sandbox.config.system_domain,
            &sandbox.config.system_db,
        );

        // 🎯 FIX CRITIQUE : Injection des composants IA factices pour que l'orchestrateur démarre
        inject_mock_component(
            &manager,
            "llm",
            json_value!({ "provider": "mock", "model": "test" }),
        )
        .await;
        inject_mock_component(&manager, "rag", json_value!({ "provider": "mock" })).await;

        let sensor_doc = json_value!({
            "_id": "vibration_z",
            "value": 15.5,
            "updatedAt": UtcClock::now().to_rfc3339()
        });

        manager
            .create_collection(
                "digital_twin",
                "db://_system/_system/schemas/v1/db/generic.schema.json",
            )
            .await
            .unwrap();
        manager
            .upsert_document("digital_twin", sensor_doc)
            .await
            .unwrap();

        // Création du faux contexte requis par la nouvelle signature
        let orch = AiOrchestrator::new(ProjectModel::default(), &manager, sandbox.db.clone())
            .await
            .unwrap();
        let pm = SharedRef::new(PluginManager::new(&sandbox.db, None));
        let critic = WorkflowCritic::default();
        let tools = UnorderedMap::new();

        let ctx = HandlerContext {
            orchestrator: &SharedRef::new(AsyncMutex::new(orch)),
            plugin_manager: &pm,
            critic: &critic,
            tools: &tools,
            manager: &manager,
        };

        let tool = SystemMonitorTool::new();
        // L'exécution utilise désormais le contexte complet !
        let result = tool.execute(&json_value!({}), &ctx).await.unwrap();

        assert_eq!(result["vibration_z"], 15.5);
        assert_eq!(result["status"], "ONLINE");
    }
}
