// FICHIER : src-tauri/src/ai/health.rs

use crate::json_db::collections::manager::CollectionsManager;
use crate::utils::prelude::*; // 🎯 Import ajouté

#[derive(Serializable, FmtDebug, PartialEq)]
pub struct HealthReport {
    pub device_type: String,
    pub acceleration_active: bool,
    pub mkl_enabled: bool,
    pub assets_integrity: bool,
    pub diagnostic_details: UnorderedMap<String, JsonValue>,
}

pub struct RaiseHealthEngine;

impl RaiseHealthEngine {
    /// Exécute un diagnostic complet (nécessite l'accès à la configuration dynamique)
    pub async fn check_engine_health(
        manager: &CollectionsManager<'_>,
    ) -> RaiseResult<HealthReport> {
        let mut details = UnorderedMap::new();

        let device = AppConfig::device();
        let (dev_type, accelerated) = match device {
            candle_core::Device::Cuda(_) => ("NVIDIA_GPU_CUDA", true),
            candle_core::Device::Metal(_) => ("APPLE_GPU_METAL", true),
            candle_core::Device::Cpu => ("SYSTEM_CPU", false),
        };

        let mkl_feat = cfg!(feature = "mkl");
        details.insert("hardware_label".to_string(), json_value!(dev_type));
        details.insert(
            "cuda_compiled".to_string(),
            json_value!(cfg!(feature = "cuda")),
        );

        // 🎯 Remplacement par notre pattern match strict
        let assets_ok = match Self::verify_critical_assets(manager, &mut details).await {
            Ok(status) => status,
            Err(e) => raise_error!(
                "ERR_HEALTH_ASSETS_DIAGNOSTIC_FAILED",
                error = "Impossible d'effectuer la vérification d'intégrité",
                context = json_value!({"technical_error": e.to_string()})
            ),
        };

        user_info!(
            "MSG_HEALTH_DIAGNOSTIC_COMPLETE",
            json_value!({
                "device": dev_type,
                "integrity": assets_ok,
                "mkl": mkl_feat
            })
        );

        Ok(HealthReport {
            device_type: dev_type.to_string(),
            acceleration_active: accelerated,
            mkl_enabled: mkl_feat,
            assets_integrity: assets_ok,
            diagnostic_details: details,
        })
    }

    async fn verify_critical_assets(
        manager: &CollectionsManager<'_>,
        logs: &mut UnorderedMap<String, JsonValue>,
    ) -> RaiseResult<bool> {
        let mut all_present = true;
        let app_config = AppConfig::get();

        let base_path = match app_config.get_path("PATH_RAISE_DOMAIN") {
            Some(path) => path.join("_system/ai-assets"),
            None => raise_error!(
                "ERR_CONFIG_PATH_MISSING",
                error = "PATH_RAISE_DOMAIN non trouvé dans la configuration",
                context = json_value!({})
            ),
        };

        // 🎯 LECTURE DYNAMIQUE DU NOM DU MODÈLE VIA LA BASE JSON
        let (model_filename, _) = match AppConfig::get_llm_settings(manager).await {
            Ok(res) => res,
            Err(e) => raise_error!(
                "ERR_HEALTH_LLM_CONFIG",
                error = "Impossible de récupérer le nom du modèle pour le diagnostic",
                context = json_value!({"technical_error": e.to_string()})
            ),
        };

        // On construit la liste des fichiers en injectant notre modèle dynamique
        let critical_files = vec![
            ("LLM_MODEL", format!("models/{}", model_filename)),
            (
                "EMB_WEIGHTS",
                "embeddings/minilm/model.safetensors".to_string(),
            ),
        ];

        for (label, rel_path) in critical_files {
            let full_path = base_path.join(&rel_path);
            let exists = fs::exists_async(&full_path).await;

            if !exists {
                all_present = false;
            }
            logs.insert(label.to_string(), json_value!(exists));
        }

        Ok(all_present)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json_db::collections::manager::CollectionsManager;
    use crate::utils::testing::mock::{inject_mock_component, AgentDbSandbox};

    #[async_test]
    async fn test_health_diagnostic_hardware_consistency() {
        let sandbox = AgentDbSandbox::new().await;
        let manager = CollectionsManager::new(
            &sandbox.db,
            &sandbox.config.system_domain,
            &sandbox.config.system_db,
        );

        // 🎯 Injection requise pour que le diagnostic puisse lire la config !
        inject_mock_component(&manager, "llm", json_value!({})).await;

        let report_result = RaiseHealthEngine::check_engine_health(&manager).await;

        match report_result {
            Ok(report) => {
                if cfg!(feature = "cuda") && report.device_type.contains("CUDA") {
                    assert!(report.acceleration_active);
                } else {
                    assert_eq!(report.mkl_enabled, cfg!(feature = "mkl"));
                }
            }
            Err(e) => panic!("Le diagnostic matériel ne devrait pas crasher : {:?}", e),
        }
    }

    #[async_test]
    async fn test_health_assets_failure_detection() {
        let sandbox = AgentDbSandbox::new().await;
        let manager = CollectionsManager::new(
            &sandbox.db,
            &sandbox.config.system_domain,
            &sandbox.config.system_db,
        );

        // 🎯 Injection requise
        inject_mock_component(&manager, "llm", json_value!({})).await;

        let report_result = RaiseHealthEngine::check_engine_health(&manager).await;

        match report_result {
            Ok(report) => {
                // Les fichiers n'étant pas dans la sandbox, le diagnostic
                // doit s'exécuter jusqu'au bout mais marquer l'intégrité à "false".
                assert!(
                    !report.assets_integrity,
                    "L'intégrité devrait être fausse en sandbox vide."
                );

                // On vérifie que la clé dynamique "LLM_MODEL" est bien présente dans les logs de diag
                assert!(report.diagnostic_details.contains_key("LLM_MODEL"));
            }
            Err(e) => panic!(
                "Le diagnostic global a crasher au lieu de remonter un statut false : {:?}",
                e
            ),
        }
    }
}
