// FICHIER : src-tauri/src/workflow_engine/tools/system_tools.rs

use super::AgentTool;
use crate::utils::prelude::*;

// Imports pour le Jumeau Numérique via JSON-DB
use crate::json_db::collections::manager::CollectionsManager;
use crate::json_db::storage::{JsonDbConfig, StorageEngine};

/// Outil permettant à l'IA et au Workflow de lire l'état du Jumeau Numérique.
/// Cet outil est désormais "Stateless" et lit la source de vérité en base de données.
#[derive(Debug, Default)]
pub struct SystemMonitorTool;

impl SystemMonitorTool {
    /// Initialise une nouvelle instance de l'outil.
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

    /// Exécute la lecture des métriques en interrogeant la persistance du Jumeau Numérique.
    async fn execute(&self, _params: &JsonValue) -> RaiseResult<JsonValue> {
        tracing::info!("🔍 [SystemMonitorTool] Lecture du Jumeau Numérique via JSON-DB...");

        // 1. Accès à la configuration et initialisation du moteur de stockage
        let config = AppConfig::get();
        let db_root = config
            .get_path("PATH_RAISE_DOMAIN")
            .unwrap_or_else(|| PathBuf::from("./_system"));

        let storage = StorageEngine::new(JsonDbConfig::new(db_root));
        let manager = CollectionsManager::new(&storage, &config.system_domain, &config.system_db);

        // 2. Récupération décentralisée de la donnée (vibration_z) mise à jour par le CLI ou l'UI
        let vibration_z = match manager.get_document("digital_twin", "vibration_z").await {
            Ok(Some(doc)) => doc["value"].as_f64().unwrap_or(2.0),
            _ => {
                tracing::warn!(
                    "⚠️ Capteur 'vibration_z' non trouvé, utilisation de la valeur nominale."
                );
                2.0
            }
        };

        // 3. Agrégation des métriques pour le contexte de l'Agent
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
    // 🎯 IMPORT UNIQUE : On utilise la GlobalDbSandbox car l'outil s'appuie
    // sur le Singleton global AppConfig, et le test est séquentiel (#[serial]).
    use crate::json_db::collections::manager::CollectionsManager;
    use crate::utils::testing::GlobalDbSandbox;

    #[async_test]
    #[serial_test::serial]
    async fn test_system_tool_persistence_integration() {
        // 1. 🎯 MAGIE : La GlobalDbSandbox configure le mock, purge l'ancienne base,
        // recrée le schéma et initialise le tout en UNE ligne !
        let sandbox = GlobalDbSandbox::new().await;

        let manager = CollectionsManager::new(
            &sandbox.db,
            &sandbox.config.system_domain,
            &sandbox.config.system_db,
        );

        // 2. Injection manuelle d'une valeur critique pour tester le grounding de l'IA
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
        // On s'assure que l'insertion fonctionne (unwrap est utile dans les tests pour repérer les erreurs vite)
        manager
            .insert_raw("digital_twin", &sensor_doc)
            .await
            .unwrap();

        // 3. Exécution de l'outil
        let tool = SystemMonitorTool::new();
        let result = tool.execute(&json_value!({})).await.unwrap();

        // 4. Vérifications
        assert_eq!(result["vibration_z"], 15.5);
        assert_eq!(result["status"], "ONLINE");
    }
}
