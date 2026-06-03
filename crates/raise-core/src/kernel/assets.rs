// FICHIER : crates/raise-core/src/kernel/assets.rs

use crate::utils::prelude::*;

pub struct AssetResolver;

impl AssetResolver {
    /// Résout un fichier avec la logique stricte de fallback (Domaine/DB -> Usine d'Assets).
    /// Nomenclature `_sync` car l'opération bloque le thread courant pour sonder le système de fichiers.
    pub fn resolve_ai_file_sync(
        primary_base_path: &Path,
        asset_category_path: &str, // ex: "models" ou "ai-assets/models"
        filename: &str,
    ) -> Option<PathBuf> {
        let config = AppConfig::get();

        // 1. Test du chemin primaire (Spécifique au domaine/db en cours)
        let primary = primary_base_path.join(filename);
        if fs::exists_sync(&primary) {
            return Some(primary);
        }

        // 2. Test du chemin partagé (Usine d'assets via PATH_RAISE_ASSET)
        // On gère gracieusement le cas où la catégorie contient déjà "ai-assets/"
        let relative_category = asset_category_path
            .strip_prefix("ai-assets/")
            .unwrap_or(asset_category_path);

        if let Some(factory_path) = config.get_path("PATH_RAISE_ASSET") {
            let shared = factory_path.join(relative_category).join(filename);

            if fs::exists_sync(&shared) {
                return Some(shared);
            }
        } else {
            // 🛡️ FALLBACK ZÉRO DETTE ABSOLU : On utilise les variables du .env pour la déduction
            let raise_domain_path = config
                .get_path("PATH_RAISE_DOMAIN")
                .unwrap_or_else(|| PathBuf::from("./raise_domain"));

            let asset_domain = crate::utils::core::RuntimeEnv::var("RAISE_ASSET_DOMAIN")
                .unwrap_or_else(|_| "_system".to_string());
            let asset_db = crate::utils::core::RuntimeEnv::var("RAISE_ASSET_DB")
                .unwrap_or_else(|_| "ai-assets".to_string());

            let shared = raise_domain_path
                .join(asset_domain)
                .join(asset_db)
                .join(relative_category)
                .join(filename);

            if fs::exists_sync(&shared) {
                return Some(shared);
            }
        }
        // 3. Introuvable
        None
    }

    /// Génère un contexte d'erreur JSON standardisé pour les logs en cas d'échec
    pub fn missing_file_context(
        primary_base_path: &Path,
        asset_category_path: &str,
        filename: &str,
    ) -> JsonValue {
        let config = AppConfig::get();
        let relative_category = asset_category_path
            .strip_prefix("ai-assets/")
            .unwrap_or(asset_category_path);

        let checked_shared = match config.get_path("PATH_RAISE_ASSET") {
            Some(factory_path) => factory_path.join(relative_category).join(filename),
            None => {
                let raise_domain_path = config
                    .get_path("PATH_RAISE_DOMAIN")
                    .unwrap_or_else(|| PathBuf::from("./raise_domain"));
                raise_domain_path
                    .join("_system")
                    .join("ai-assets")
                    .join(relative_category)
                    .join(filename)
            }
        };

        json_value!({
            "filename": filename,
            "checked_primary": primary_base_path.join(filename).to_string_lossy(),
            "checked_shared": checked_shared.to_string_lossy()
        })
    }
}

// =========================================================================
// TESTS UNITAIRES (Validation Stricte de la Cascade & Façade FS)
// =========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::testing::AgentDbSandbox;

    /// Helper local qui respecte ABSOLUMENT la façade Raise
    fn touch_test_file(path: &Path) -> RaiseResult<()> {
        if let Some(parent) = path.parent() {
            fs::ensure_dir_sync(parent)?;
        }
        fs::write_sync(path, b"dummy data")?;
        Ok(())
    }

    #[async_test]
    #[serial_test::serial]
    async fn test_resolver_priority_primary_over_shared() -> RaiseResult<()> {
        let _sandbox = AgentDbSandbox::new().await?;
        let config = AppConfig::get();

        let raise_domain_path = config.get_path("PATH_RAISE_DOMAIN").unwrap();
        let primary_path = raise_domain_path
            .join("test_domain")
            .join("test_db")
            .join("models"); // Utilisation d'un sous-dossier standard
        let category = "models";
        let filename = "qwen_test.gguf";

        let expected_primary = primary_path.join(filename);

        // On calcule où le test attend le shared fallback
        let expected_shared = if let Some(factory_path) = config.get_path("PATH_RAISE_ASSET") {
            factory_path.join(category).join(filename)
        } else {
            raise_domain_path
                .join("_system")
                .join("ai-assets")
                .join(category)
                .join(filename)
        };

        // On crée les DEUX fichiers
        touch_test_file(&expected_primary)?;
        touch_test_file(&expected_shared)?;

        // Exécution via la façade
        let resolved = AssetResolver::resolve_ai_file_sync(&primary_path, category, filename);

        assert!(resolved.is_some(), "Le fichier devrait être résolu");
        assert_eq!(
            resolved.unwrap(),
            expected_primary,
            "Le résolveur DOIT prioriser le chemin primaire"
        );

        // Nettoyage via la façade
        let _ = fs::remove_file_sync(&expected_primary);
        let _ = fs::remove_file_sync(&expected_shared);
        Ok(())
    }

    #[async_test]
    #[serial_test::serial]
    async fn test_resolver_fallback_to_shared() -> RaiseResult<()> {
        let _sandbox = AgentDbSandbox::new().await?;
        let config = AppConfig::get();

        let raise_domain_path = config.get_path("PATH_RAISE_DOMAIN").unwrap();
        let primary_path = raise_domain_path
            .join("test_domain")
            .join("test_db")
            .join("models");

        // On teste avec le préfixe pour valider le nettoyage automatique de "ai-assets/"
        let category = "ai-assets/models";
        let filename = "whisper_test.bin";

        let expected_primary = primary_path.join(filename);
        let expected_shared = if let Some(factory_path) = config.get_path("PATH_RAISE_ASSET") {
            factory_path.join("models").join(filename) // Le test valide que "models" est bien utilisé
        } else {
            raise_domain_path
                .join("_system")
                .join("ai-assets")
                .join("models")
                .join(filename)
        };

        if fs::exists_sync(&expected_primary) {
            let _ = fs::remove_file_sync(&expected_primary);
        }
        touch_test_file(&expected_shared)?;

        let resolved = AssetResolver::resolve_ai_file_sync(&primary_path, category, filename);

        assert!(
            resolved.is_some(),
            "Le fichier devrait être résolu via le fallback"
        );
        assert_eq!(
            resolved.unwrap(),
            expected_shared,
            "Le résolveur aurait dû basculer sur le chemin partagé"
        );

        let _ = fs::remove_file_sync(&expected_shared);
        Ok(())
    }

    #[async_test]
    #[serial_test::serial]
    async fn test_resolver_file_not_found() -> RaiseResult<()> {
        let _sandbox = AgentDbSandbox::new().await?;
        let config = AppConfig::get();
        let raise_domain_path = config.get_path("PATH_RAISE_DOMAIN").unwrap();

        let primary_path = raise_domain_path.join("ghost_domain").join("ghost_db");
        let category = "ghost_category";
        let filename = "ghost_model.gguf";

        let resolved = AssetResolver::resolve_ai_file_sync(&primary_path, category, filename);

        assert!(
            resolved.is_none(),
            "Le résolveur doit retourner None si le fichier n'existe nulle part"
        );
        Ok(())
    }

    #[async_test]
    #[serial_test::serial]
    async fn test_resolver_missing_context_format() -> RaiseResult<()> {
        let _sandbox = AgentDbSandbox::new().await?;
        let config = AppConfig::get();

        let primary_path = PathBuf::from("/mock/primary/path");
        let category = "ai-assets/models";
        let filename = "test.json";

        let context = AssetResolver::missing_file_context(&primary_path, category, filename);

        assert_eq!(context["filename"], "test.json");
        assert!(context["checked_primary"]
            .as_str()
            .unwrap()
            .contains("/mock/primary/path/test.json"));

        // Le chemin testé dépendra de si PATH_RAISE_ASSET est mocké ou non
        let shared_str = context["checked_shared"].as_str().unwrap();
        if config.get_path("PATH_RAISE_ASSET").is_some() {
            assert!(shared_str.contains("models/test.json"));
        } else {
            assert!(shared_str.contains("_system/ai-assets/models/test.json"));
        }
        Ok(())
    }
}
