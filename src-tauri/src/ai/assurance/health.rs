// FICHIER : src-tauri/src/ai/assurance/health.rs

use crate::json_db::collections::manager::CollectionsManager;
use crate::utils::prelude::*; // 🎯 Façade Unique

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
    /// Exécute un diagnostic complet du moteur d'IA (Hardware + Assets)
    pub async fn check_engine_health(
        manager: &CollectionsManager<'_>,
    ) -> RaiseResult<HealthReport> {
        let mut details = UnorderedMap::new();

        // 1. Diagnostic Hardware via AppConfig (SSOT)
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

        // 2. Diagnostic des Assets (Modèles & Tokenizers)
        // 🎯 Rigueur : Pattern match strict sur la vérification d'intégrité
        let assets_ok = match Self::verify_critical_assets(manager, &mut details).await {
            Ok(status) => status,
            Err(e) => {
                raise_error!(
                    "ERR_HEALTH_ASSETS_DIAGNOSTIC_FAILED",
                    error = "Impossible d'effectuer la vérification d'intégrité des actifs d'IA.",
                    context = json_value!({"technical_error": e.to_string()})
                );
            }
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

    /// Vérifie la présence physique des poids des modèles sur le disque
    async fn verify_critical_assets(
        manager: &CollectionsManager<'_>,
        logs: &mut UnorderedMap<String, JsonValue>,
    ) -> RaiseResult<bool> {
        let mut all_present = true;
        let app_config = AppConfig::get();

        // 🎯 FIX MOUNT POINTS : Utilisation du point de montage Système configuré
        let base_path = match app_config.get_path("PATH_RAISE_DOMAIN") {
            Some(path) => {
                // Résolution déterministe vers la partition système
                path.join(&app_config.mount_points.system.domain)
                    .join(&app_config.mount_points.system.db)
                    .join("ai-assets")
            }
            None => {
                raise_error!(
                    "ERR_CONFIG_PATH_MISSING",
                    error = "PATH_RAISE_DOMAIN non trouvé dans la configuration globale.",
                    context = json_value!({"action": "resolve_ai_assets_path"})
                );
            }
        };

        // 🎯 LECTURE DYNAMIQUE : On récupère les fichiers configurés en base
        let (model_filename, _) = match AppConfig::get_llm_settings(manager).await {
            Ok(res) => res,
            Err(e) => {
                raise_error!(
                    "ERR_HEALTH_LLM_CONFIG",
                    error =
                        "Impossible de récupérer la configuration LLM pour le diagnostic matériel.",
                    context = json_value!({"technical_error": e.to_string()})
                );
            }
        };

        // Liste des fichiers critiques pour le fonctionnement nominal
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
                user_warn!(
                    "WRN_HEALTH_ASSET_MISSING",
                    json_value!({ "asset": label, "path": full_path.to_string_lossy() })
                );
            }
            logs.insert(label.to_string(), json_value!(exists));
        }

        Ok(all_present)
    }
}

// =========================================================================
// TESTS UNITAIRES (Rigueur Façade & Résilience)
// =========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::testing::mock::{inject_mock_component, AgentDbSandbox};

    #[async_test]
    async fn test_health_diagnostic_hardware_consistency() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await;
        let config = AppConfig::get();

        // Point de montage système pour la lecture de config
        let manager = CollectionsManager::new(
            &sandbox.db,
            &config.mount_points.system.domain,
            &config.mount_points.system.db,
        );

        // Injection du composant requis pour le diagnostic
        inject_mock_component(&manager, "llm", json_value!({})).await;

        let report = RaiseHealthEngine::check_engine_health(&manager).await?;

        // Validation de la cohérence Hardware
        if cfg!(feature = "cuda") && report.device_type.contains("CUDA") {
            assert!(report.acceleration_active);
        } else {
            assert_eq!(report.mkl_enabled, cfg!(feature = "mkl"));
        }

        Ok(())
    }

    #[async_test]
    async fn test_health_assets_failure_detection() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await;
        let config = AppConfig::get();
        let manager = CollectionsManager::new(
            &sandbox.db,
            &config.mount_points.system.domain,
            &config.mount_points.system.db,
        );

        inject_mock_component(&manager, "llm", json_value!({})).await;

        let report = RaiseHealthEngine::check_engine_health(&manager).await?;

        // Dans une sandbox vide, les fichiers physiques n'existent pas
        assert!(
            !report.assets_integrity,
            "L'intégrité devrait être fausse car les modèles ne sont pas présents sur le disque."
        );

        assert!(report.diagnostic_details.contains_key("LLM_MODEL"));

        Ok(())
    }

    // 🎯 NOUVEAU TEST : Résilience face à une config LLM manquante
    #[async_test]
    async fn test_health_resilience_on_config_error() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await;
        let config = AppConfig::get();

        // Manager sur une base vide sans injection de "llm"
        let manager = CollectionsManager::new(
            &sandbox.db,
            &config.mount_points.system.domain,
            &config.mount_points.system.db,
        );

        let result = RaiseHealthEngine::check_engine_health(&manager).await;

        // Le diagnostic doit retourner une erreur structurée plutôt que de paniquer
        match result {
            Err(AppError::Structured(data)) => {
                assert!(
                    data.code.contains("ERR_HEALTH_LLM_CONFIG")
                        || data.code.contains("ERR_HEALTH_ASSETS_DIAGNOSTIC_FAILED")
                );
                Ok(())
            }
            _ => panic!("Le moteur aurait dû lever une erreur de configuration LLM"),
        }
    }
}
