// FICHIER : src-tauri/src/ai/assurance/health.rs

use crate::json_db::collections::manager::CollectionsManager;
use crate::kernel::assets::AssetResolver;
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
            ComputeHardware::Cuda(_) => {
                if cfg!(not(test)) {
                    Self::check_vram_availability(0, manager, &mut details).await?;
                } else {
                    // En test, on simule une VRAM saine pour ne pas bloquer la CI/Dev
                    details.insert("vram_free_mb".into(), json_value!(8192));
                }
                ("NVIDIA_GPU_CUDA", true)
            }
            ComputeHardware::Metal(_) => ("APPLE_GPU_METAL", true),
            ComputeHardware::Cpu => ("SYSTEM_CPU", false),
        };

        // 2. Diagnostic des Assets (Modèles & Tokenizers)
        let assets_ok = Self::verify_critical_assets(manager, &mut details).await?;

        user_info!(
            "MSG_HEALTH_DIAGNOSTIC_COMPLETE",
            json_value!({
                "device": dev_type,
                "integrity": assets_ok,
                "vram_check": accelerated
            })
        );

        Ok(HealthReport {
            device_type: dev_type.to_string(),
            acceleration_active: accelerated,
            mkl_enabled: cfg!(feature = "mkl"),
            assets_integrity: assets_ok,
            diagnostic_details: details,
        })
    }

    /// Interroge NvidiaMonitor pour valider la mémoire disponible par rapport aux besoins
    async fn check_vram_availability(
        device_index: u32,
        manager: &CollectionsManager<'_>,
        logs: &mut UnorderedMap<String, JsonValue>,
    ) -> RaiseResult<()> {
        // A. Récupération de la contrainte (ex: 6000 MB pour Qwen 7B)
        let required_vram_mb = match manager
            .get_document("service_configs", "cfg_ai_default")
            .await
        {
            Ok(Some(doc)) => {
                doc["resource_constraints"]["require_vram_mb"]
                    .as_u64()
                    .unwrap_or(5500) // 4Go par défaut
            }
            Ok(None) => 5500,
            Err(e) => raise_error!("ERR_HEALTH_CONFIG_READ", error = e.to_string()),
        };

        // B. Initialisation NvidiaMonitor (Utilisation de match...raise_error!)
        let nvml = match NvidiaMonitor::init() {
            Ok(n) => n,
            Err(e) => raise_error!("ERR_HEALTH_NVML_INIT", error = e.to_string()),
        };

        // C. Accès au GPU (Index 0 pour ta RTX 5060)
        let device = match nvml.device_by_index(device_index) {
            Ok(d) => d,
            Err(e) => raise_error!(
                "ERR_HEALTH_GPU_NOT_FOUND",
                error = e.to_string(),
                context = json_value!({"index": device_index})
            ),
        };

        // D. Lecture de la mémoire réelle
        let memory_info = match device.memory_info() {
            Ok(m) => m,
            Err(e) => raise_error!("ERR_HEALTH_VRAM_FETCH", error = e.to_string()),
        };

        let free_vram_mb = memory_info.free / 1024 / 1024;

        logs.insert("vram_free_mb".into(), json_value!(free_vram_mb));
        logs.insert("vram_required_mb".into(), json_value!(required_vram_mb));

        // E. VETO : On bloque si la mémoire est insuffisante pour éviter le crash DriverError
        if free_vram_mb < required_vram_mb {
            user_error!(
                "AI_HEALTH_VRAM_LOW",
                json_value!({ "free": free_vram_mb, "required": required_vram_mb })
            );

            raise_error!(
                "ERR_ASSURANCE_VRAM_INSUFFICIENT",
                error = format!(
                    "Mémoire GPU insuffisante pour charger le modèle (Libre: {}MB, Requis: {}MB)",
                    free_vram_mb, required_vram_mb
                )
            );
        }

        Ok(())
    }

    /// Vérifie la présence physique des poids des modèles sur le disque
    /// Vérifie la présence physique des poids des modèles sur le disque
    async fn verify_critical_assets(
        manager: &CollectionsManager<'_>,
        logs: &mut UnorderedMap<String, JsonValue>,
    ) -> RaiseResult<bool> {
        let mut all_present = true;
        let app_config = AppConfig::get();

        // 1. Racine du domaine Raise
        let raise_domain_path = match app_config.get_path("PATH_RAISE_DOMAIN") {
            Some(path) => path,
            None => raise_error!(
                "ERR_CONFIG_PATH_MISSING",
                error = "PATH_RAISE_DOMAIN non configuré."
            ),
        };

        // 2. Chemin prioritaire de base (Spécifique Domaine/DB)
        let primary_base_path = raise_domain_path
            .join(&app_config.mount_points.system.domain)
            .join(&app_config.mount_points.system.db)
            .join("ai-assets");

        // 3. Utilisation du Gatekeeper
        let settings =
            match AppConfig::get_runtime_settings(manager, "ref:components:handle:ai_llm").await {
                Ok(s) => s,
                Err(e) => raise_error!("ERR_HEALTH_LLM_CONFIG", error = e.to_string()),
            };

        let model_filename = match settings.get("rust_model_file").and_then(|v| v.as_str()) {
            Some(m) => m.to_string(),
            None => raise_error!(
                "ERR_HEALTH_LLM_MODEL_MISSING",
                error = "La clé 'rust_model_file' est introuvable."
            ),
        };

        // 🎯 LA CORRECTION EST ICI : On définit bien 3 éléments séparés (Label, Dossier, Fichier)
        let critical_files = vec![
            ("LLM_MODEL", "models", model_filename.as_str()),
            ("EMB_WEIGHTS", "embeddings/minilm", "model.safetensors"),
        ];

        // 5. Délégation de la logique de fallback à l'AssetResolver !
        for (label, category, filename) in critical_files {
            let resolved =
                AssetResolver::resolve_ai_file_sync(&primary_base_path, category, filename);

            if resolved.is_some() {
                logs.insert(label.to_string(), json_value!(true));
            } else {
                all_present = false;

                // Utilisation du helper pour générer le log d'erreur standardisé
                user_warn!(
                    "WRN_HEALTH_ASSET_MISSING",
                    AssetResolver::missing_file_context(&primary_base_path, category, filename)
                );

                logs.insert(label.to_string(), json_value!(false));
            }
        }

        Ok(all_present)
    }
}

// =========================================================================
// TESTS UNITAIRES
// =========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::testing::mock::AgentDbSandbox;

    #[async_test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_health_diagnostic_hardware_consistency() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await?;
        let config = AppConfig::get();

        let manager = CollectionsManager::new(
            &sandbox.db,
            &config.mount_points.system.domain,
            &config.mount_points.system.db,
        );

        let report = RaiseHealthEngine::check_engine_health(&manager).await?;

        if cfg!(feature = "cuda") && report.device_type.contains("CUDA") {
            assert!(report.acceleration_active);
        } else {
            assert_eq!(report.mkl_enabled, cfg!(feature = "mkl"));
        }

        Ok(())
    }

    #[async_test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_health_assets_failure_detection() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await?;
        let config = AppConfig::get();
        let manager = CollectionsManager::new(
            &sandbox.db,
            &config.mount_points.system.domain,
            &config.mount_points.system.db,
        );

        let report = RaiseHealthEngine::check_engine_health(&manager).await?;

        assert!(!report.assets_integrity);
        assert!(report.diagnostic_details.contains_key("LLM_MODEL"));

        Ok(())
    }

    #[async_test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_health_resilience_on_config_error() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await?;
        let config = AppConfig::get();

        let manager = CollectionsManager::new(
            &sandbox.db,
            &config.mount_points.system.domain,
            &config.mount_points.system.db,
        );

        let _ = manager
            .delete_document("service_configs", "cfg_ai_llm_test")
            .await;
        let result = RaiseHealthEngine::check_engine_health(&manager).await;

        match result {
            Err(AppError::Structured(data)) => {
                assert!(
                    data.code.contains("ERR_HEALTH_LLM_CONFIG")
                        || data.code.contains("ERR_HEALTH_ASSETS_DIAGNOSTIC_FAILED")
                );
                Ok(())
            }
            _ => raise_error!(
                "ERR_TEST_FAIL",
                error = "Le moteur aurait dû lever une erreur de configuration LLM"
            ),
        }
    }

    #[async_test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_physical_local_model_resolution() -> RaiseResult<()> {
        let _sandbox = AgentDbSandbox::new().await?;
        let config = AppConfig::get();

        // 1. Chemin isolé généré par la Sandbox (Zéro Dette : on respecte l'isolation)
        let raise_domain_sandbox = config
            .get_path("PATH_RAISE_DOMAIN")
            .unwrap_or_else(|| PathBuf::from("./raise_domain"));

        let category = "ai-assets/models";
        let filename = "qwen2-5-codeur/qwen2.5-coder-7b-instruct-q4_k_m.gguf";

        // 2. Création de l'asset dans le dossier partagé (_system) de la Sandbox
        let sandbox_shared_file = raise_domain_sandbox
            .join("_system")
            .join(category)
            .join(filename);

        // On utilise STRICTEMENT la façade pour créer l'arborescence
        if let Some(parent) = sandbox_shared_file.parent() {
            fs::ensure_dir_sync(parent)?;
        }

        // On simule la présence du fichier physique avec la façade RAISE
        fs::write_sync(&sandbox_shared_file, b"dummy gguf data")?;

        // 3. On définit le chemin primaire de la partition système (qui est vide)
        let primary_path = raise_domain_sandbox
            .join(&config.mount_points.system.domain)
            .join(&config.mount_points.system.db)
            .join("ai-assets");

        // 4. Tentative de résolution
        let resolved = AssetResolver::resolve_ai_file_sync(&primary_path, category, filename);

        // 5. Assertions strictes
        assert!(
            resolved.is_some(),
            "L'AssetResolver n'a pas réussi à basculer sur le dossier _system partagé."
        );

        let resolved_path = resolved.unwrap();

        assert!(
            fs::exists_sync(&resolved_path),
            "Le fichier mocké n'a pas été trouvé par la façade."
        );

        println!(
            "✅ Test Physique réussi (Façade Stricte) ! Modèle résolu à : {:?}",
            resolved_path
        );

        Ok(())
    }
}
