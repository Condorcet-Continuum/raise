use crate::utils::prelude::*;

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
    /// Exécute un diagnostic complet du moteur IA (Hardware + Assets)
    pub async fn check_engine_health() -> RaiseResult<HealthReport> {
        let mut details = UnorderedMap::new();

        // 1. Diagnostic Matériel (via le Singleton AppConfig)
        let device = AppConfig::device();
        let (dev_type, accelerated) = match device {
            candle_core::Device::Cuda(_) => ("NVIDIA_GPU_CUDA", true),
            candle_core::Device::Metal(_) => ("APPLE_GPU_METAL", true),
            candle_core::Device::Cpu => ("SYSTEM_CPU", false),
        };

        // 2. Détection des features de compilation (MKL/CUDA/METAL)
        let mkl_feat = cfg!(feature = "mkl");
        details.insert("hardware_label".to_string(), json_value!(dev_type));
        details.insert(
            "cuda_compiled".to_string(),
            json_value!(cfg!(feature = "cuda")),
        );

        // 3. Vérification de l'intégrité des fichiers (Assets)
        let assets_ok = Self::verify_critical_assets(&mut details).await?;

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

    /// Vérifie la présence physique des poids du modèle Qwen et BERT
    async fn verify_critical_assets(
        logs: &mut UnorderedMap<String, JsonValue>,
    ) -> RaiseResult<bool> {
        let mut all_present = true;
        let app_config = AppConfig::get();

        let base_path = match app_config.get_path("PATH_RAISE_DOMAIN") {
            Some(path) => path.join("_system/ai-assets"),
            None => raise_error!(
                "ERR_CONFIG_PATH_MISSING",
                error = "PATH_RAISE_DOMAIN non trouvé",
                context = json_value!({
                    "action": "resolve_critical_assets_path",
                    "hint": "Vérifiez que la configuration active contient bien le chemin racine du domaine."
                })
            ),
        };

        let critical_files = vec![
            ("LLM_MODEL", "models/qwen2.5-1.5b-instruct-q4_k_m.gguf"),
            ("EMB_WEIGHTS", "embeddings/minilm/model.safetensors"),
        ];

        for (label, rel_path) in critical_files {
            let full_path = base_path.join(rel_path);
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
    use crate::utils::testing::AgentDbSandbox;

    #[async_test]
    async fn test_health_diagnostic_hardware_consistency() {
        // Initialisation de la sandbox (Mocks config + device)
        let _sandbox = AgentDbSandbox::new().await;

        let report = RaiseHealthEngine::check_engine_health()
            .await
            .expect("Le diagnostic ne devrait pas crasher");

        // Sur ton PC AMD, on s'attend à du CPU (ou CUDA si compilé)
        if cfg!(feature = "cuda") && report.device_type.contains("CUDA") {
            assert!(report.acceleration_active);
        } else {
            // Si CPU, on vérifie si MKL est bien là pour la performance
            assert_eq!(report.mkl_enabled, cfg!(feature = "mkl"));
        }
    }

    #[async_test]
    async fn test_health_assets_failure_detection() {
        let _sandbox = AgentDbSandbox::new().await;

        // Ce test va probablement échouer car les fichiers ne sont pas
        // dans le dossier temporaire de la sandbox.
        let report = RaiseHealthEngine::check_engine_health().await.unwrap();

        // On vérifie que le moteur détecte bien l'absence d'assets dans un env vide
        assert!(!report.assets_integrity);
        assert!(report.diagnostic_details.contains_key("LLM_MODEL"));
    }
}
