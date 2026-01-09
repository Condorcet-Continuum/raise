// FICHIER : src-tauri/src/workflow_engine/tools/system_tools.rs

use super::AgentTool;
use crate::utils::Result;
use serde_json::{json, Value};

#[derive(Debug)]
pub struct SystemMonitorTool;

#[async_trait::async_trait]
impl AgentTool for SystemMonitorTool {
    fn name(&self) -> &str {
        "read_system_metrics"
    }

    fn description(&self) -> &str {
        "Lit les métriques du système (CPU, RAM) et les capteurs simulés (Vibration, Température)."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "sensor_id": {
                    "type": "string",
                    "description": "ID du capteur (ex: 'vibration_z', 'cpu_temp')"
                }
            }
        })
    }

    async fn execute(&self, args: &Value) -> Result<Value> {
        // Extraction de l'argument 'sensor_id'
        let sensor_id = args
            .get("sensor_id")
            .and_then(|v| v.as_str())
            .unwrap_or("cpu");

        tracing::info!("🔌 Accès matériel : Lecture du capteur '{}'", sensor_id);

        // Simulation de lecture hardware (Hardware Abstraction Layer)
        match sensor_id {
            "vibration_z" => {
                // Pour la démo : On renvoie une valeur critique (12.5) > Seuil (8.0)
                // Cela permettra de déclencher le VETO dans le nœud suivant.
                Ok(json!({
                    "value": 12.5,
                    "unit": "mm/s",
                    "status": "CRITICAL",
                    "timestamp": chrono::Utc::now().to_rfc3339()
                }))
            }
            "cpu_temp" => Ok(json!({
                "value": 45.0,
                "unit": "C",
                "status": "NORMAL"
            })),
            _ => {
                // Fallback ou erreur si capteur inconnu
                Ok(json!({
                    "error": "Sensor not found",
                    "available_sensors": ["vibration_z", "cpu_temp"]
                }))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore = "Simule un appel matériel externe"]
    async fn test_sensor_vibration_critical() {
        // Ce test valide le scénario de VETO
        let tool = SystemMonitorTool;
        let args = json!({ "sensor_id": "vibration_z" });

        let result = tool.execute(&args).await.unwrap();

        // Vérifications strictes pour garantir que le GatePolicy fonctionnera
        assert!(result.get("value").is_some(), "Doit retourner une valeur");
        assert_eq!(result["value"].as_f64(), Some(12.5));
        assert_eq!(result["status"], "CRITICAL");
        assert_eq!(result["unit"], "mm/s");
    }

    #[tokio::test]
    #[ignore = "Simule un appel matériel externe"]
    async fn test_sensor_cpu_normal() {
        let tool = SystemMonitorTool;
        let args = json!({ "sensor_id": "cpu_temp" });

        let result = tool.execute(&args).await.unwrap();

        assert_eq!(result["value"].as_f64(), Some(45.0));
        assert_eq!(result["status"], "NORMAL");
    }

    #[tokio::test]
    #[ignore = "Simule un appel matériel externe"]
    async fn test_unknown_sensor_handling() {
        let tool = SystemMonitorTool;
        let args = json!({ "sensor_id": "flux_capacitor" });

        let result = tool.execute(&args).await.unwrap();

        assert!(result.get("error").is_some(), "Doit signaler une erreur");
        assert_eq!(result["error"], "Sensor not found");
    }

    #[tokio::test]
    #[ignore = "Simule un appel matériel externe"]
    async fn test_metadata() {
        let tool = SystemMonitorTool;
        assert_eq!(tool.name(), "read_system_metrics");
        assert!(!tool.description().is_empty());
    }
}
