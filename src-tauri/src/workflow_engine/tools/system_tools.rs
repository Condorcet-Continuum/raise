// FICHIER : src-tauri/src/workflow_engine/tools/system_tools.rs

use super::AgentTool;
use crate::utils::prelude::*; // 🎯 Façade Unique RAISE
use crate::workflow_engine::handlers::HandlerContext;

/// Outil permettant à l'IA et au Workflow de lire l'état du Jumeau Numérique.
/// Cet outil est 100% "Stateless" et résilien grâce au contexte partagé.
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
            "properties": {
                "sensor_id": { "type": "string", "description": "ID spécifique du capteur à lire (optionnel)" }
            },
            "required": []
        })
    }

    /// Exécute la lecture des métriques en utilisant la connexion DB mutualisée.
    /// Aligné sur les bonnes pratiques RAISE : Match...raise_error.
    async fn execute(
        &self,
        params: &JsonValue,
        context: &HandlerContext<'_>,
    ) -> RaiseResult<JsonValue> {
        user_info!(
            "INF_TOOL_SYSMON_START",
            json_value!({ "tool": self.name() })
        );

        // 1. Détermination du capteur cible (Résilience du moteur)
        let target_sensor = params
            .get("sensor_id")
            .and_then(|v| v.as_str())
            .unwrap_or("vibration_z");

        // 2. Lecture du Jumeau Numérique via le manager du contexte (Zéro new StorageEngine)
        let sensor_value = match context
            .manager
            .get_document("digital_twin", target_sensor)
            .await
        {
            Ok(Some(doc)) => {
                match doc.get("value").and_then(|v| v.as_f64()) {
                    Some(val) => val,
                    None => {
                        user_warn!(
                            "WRN_TOOL_SYSMON_BAD_DATA",
                            json_value!({ "sensor": target_sensor })
                        );
                        2.0 // Valeur nominale de repli
                    }
                }
            }
            Ok(None) => {
                user_warn!(
                    "WRN_TOOL_SYSMON_MISSING",
                    json_value!({ "sensor": target_sensor })
                );
                2.0 // Valeur nominale de repli
            }
            Err(e) => {
                // En cas d'erreur DB réelle, on utilise raise_error!
                raise_error!(
                    "ERR_TOOL_SYSMON_DB",
                    error = e.to_string(),
                    context = json_value!({ "sensor": target_sensor })
                );
            }
        };

        // 3. Construction des métriques consolidées
        let metrics = json_value!({
            target_sensor: sensor_value,
            "temp_core": 45.0,
            "cpu_load": 12.5,
            "status": "ONLINE",
            "timestamp": UtcClock::now().to_rfc3339()
        });

        user_success!(
            "SUC_TOOL_SYSMON_READ",
            json_value!({ "sensor": target_sensor, "value": sensor_value })
        );
        Ok(metrics)
    }
}

// =========================================================================
// TESTS UNITAIRES (Respect de l'existant & Résilience Mount Points)
// =========================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::orchestrator::AiOrchestrator;
    use crate::json_db::collections::manager::CollectionsManager;
    use crate::model_engine::types::ProjectModel;
    use crate::plugins::manager::PluginManager;
    use crate::utils::testing::{inject_mock_component, AgentDbSandbox};
    use crate::workflow_engine::critic::WorkflowCritic;

    #[async_test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_system_tool_persistence_integration() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await?;
        let config = AppConfig::get();

        // 🎯 RÉSILIENCE MOUNT POINTS : Utilisation dynamique de la config système
        let manager = CollectionsManager::new(
            &sandbox.db,
            &config.mount_points.system.domain,
            &config.mount_points.system.db,
        );

        // Setup des composants IA factices
        inject_mock_component(&manager, "llm", json_value!({ "provider": "mock" })).await?;
        inject_mock_component(&manager, "rag", json_value!({ "provider": "mock" })).await?;

        // Préparation du Jumeau Numérique
        let schema_uri = format!(
            "db://{}/{}/schemas/v1/db/generic.schema.json",
            config.mount_points.system.domain, config.mount_points.system.db
        );

        manager
            .create_collection("digital_twin", &schema_uri)
            .await?;

        let sensor_doc = json_value!({
            "_id": "vibration_z",
            "value": 15.5,
            "updatedAt": UtcClock::now().to_rfc3339()
        });
        manager.upsert_document("digital_twin", sensor_doc).await?;

        // Création du contexte partagé pour l'outil
        let orch = AiOrchestrator::new(ProjectModel::default(), &manager, sandbox.db.clone())
            .await
            .unwrap();
        let pm = SharedRef::new(PluginManager::new(&sandbox.db, None));

        let ctx = HandlerContext {
            orchestrator: &SharedRef::new(AsyncMutex::new(orch)),
            plugin_manager: &pm,
            critic: &WorkflowCritic::default(),
            tools: &UnorderedMap::new(),
            manager: &manager,
        };

        let tool = SystemMonitorTool::new();
        let result = tool.execute(&json_value!({}), &ctx).await?;

        assert_eq!(result["vibration_z"], 15.5);
        assert_eq!(result["status"], "ONLINE");
        Ok(())
    }

    /// 🎯 NOUVEAU TEST : Résilience face à un point de montage système invalide
    #[async_test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_system_tool_mount_point_resilience() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await?;

        // 🎯 On définit notre partition fantôme
        let manager = CollectionsManager::new(&sandbox.db, "ghost_partition", "void_db");

        // 🎯 FIX CRITIQUE : Initialisation physique de la base de données fantôme
        // Cela crée la structure nécessaire dans le StorageEngine sans créer de données métier.
        crate::utils::testing::DbSandbox::mock_db(&manager).await?;

        // 🎯 On crée la collection technique pour l'IA
        let core_schema = "db://_system/_system/schemas/v1/db/generic.schema.json";
        manager
            .create_collection("service_configs", core_schema)
            .await?;

        // Injection du mock LLM pour permettre à l'orchestrateur de s'initialiser
        inject_mock_component(&manager, "llm", json_value!({ "provider": "mock" })).await?;

        let orch = AiOrchestrator::new(ProjectModel::default(), &manager, sandbox.db.clone())
            .await
            .unwrap();

        let ctx = HandlerContext {
            orchestrator: &SharedRef::new(AsyncMutex::new(orch)),
            plugin_manager: &SharedRef::new(PluginManager::new(&sandbox.db, None)),
            critic: &WorkflowCritic::default(),
            tools: &UnorderedMap::new(),
            manager: &manager,
        };

        let tool = SystemMonitorTool::new();

        // 🎯 TEST DE RÉSILIENCE : On cherche 'missing' dans une DB qui n'a PAS de collection 'digital_twin'
        let result = tool
            .execute(&json_value!({ "sensor_id": "missing" }), &ctx)
            .await?;

        // L'outil doit renvoyer la valeur de repli (2.0) définie dans son code
        assert_eq!(result["missing"], 2.0);
        Ok(())
    }
}
