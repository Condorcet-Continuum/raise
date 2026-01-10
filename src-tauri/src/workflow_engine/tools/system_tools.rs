// FICHIER : src-tauri/src/workflow_engine/tools/system_tools.rs

use super::AgentTool;
use crate::utils::Result;
use serde_json::{json, Value};
use std::sync::Mutex;

// --- JUMEAU NUMÉRIQUE (État Global Simulé) ---
// Cette variable est accessible publiquement pour être modifiée par les commandes Tauri
pub static VIBRATION_SENSOR: Mutex<f64> = Mutex::new(0.0);

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
                // MODIFICATION : Lecture dynamique depuis le Jumeau Numérique
                let lock = VIBRATION_SENSOR.lock().unwrap();
                let value = *lock; // On copie la valeur

                Ok(json!({
                    "value": value,
                    "unit": "mm/s",
                    "status": if value > 8.0 { "CRITICAL" } else { "NOMINAL" },
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
    async fn test_sensor_vibration_dynamic() {
        // Test du Jumeau Numérique
        let tool = SystemMonitorTool;
        let args = json!({ "sensor_id": "vibration_z" });

        // 1. On règle le capteur sur une valeur sûre
        {
            let mut lock = VIBRATION_SENSOR.lock().unwrap();
            *lock = 2.0;
        }
        let res_safe = tool.execute(&args).await.unwrap();
        assert_eq!(res_safe["value"].as_f64(), Some(2.0));
        assert_eq!(res_safe["status"], "NOMINAL");

        // 2. On règle le capteur sur une valeur critique
        {
            let mut lock = VIBRATION_SENSOR.lock().unwrap();
            *lock = 12.5;
        }
        let res_crit = tool.execute(&args).await.unwrap();
        assert_eq!(res_crit["value"].as_f64(), Some(12.5));
        assert_eq!(res_crit["status"], "CRITICAL");
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
}
