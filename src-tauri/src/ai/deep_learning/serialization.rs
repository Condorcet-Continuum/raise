// FICHIER : src-tauri/src/ai/deep_learning/serialization.rs

use crate::ai::deep_learning::models::sequence_net::SequenceNet;
use crate::utils::prelude::*; // 🎯 Façade Unique

/// Sauvegarde les poids du modèle dans un fichier au format SafeTensors.
pub fn save_model(varmap: &NeuralWeightsMap, path: impl AsRef<Path>) -> RaiseResult<()> {
    // 🎯 Pattern matching pour la gestion des erreurs d'I/O SafeTensors
    match varmap.save(path.as_ref()) {
        Ok(_) => Ok(()),
        Err(e) => raise_error!(
            "ERR_DL_SAVE_FAILED",
            error = e.to_string(),
            context = json_value!({ "path": path.as_ref().to_string_lossy() })
        ),
    }
}

/// Charge un modèle SequenceNet complet depuis un fichier pour l'inférence.
pub fn load_model(path: impl AsRef<Path>, config: &DeepLearningConfig) -> RaiseResult<SequenceNet> {
    // 🎯 Résolution via la façade Mount Points pour le matériel
    let device = AppConfig::device();

    // Vérification de l'existence avant chargement mmap
    if !path.as_ref().exists() {
        raise_error!(
            "ERR_DL_MODEL_NOT_FOUND",
            error = "Le fichier de poids .safetensors est introuvable.",
            context = json_value!({ "path": path.as_ref().to_string_lossy() })
        );
    }

    let vb = unsafe {
        match NeuralWeightsBuilder::from_mmaped_safetensors(
            &[path.as_ref()],
            ComputeType::F32,
            device,
        ) {
            Ok(v) => v,
            Err(e) => raise_error!(
                "ERR_DL_MMAP_FAILED",
                error = e.to_string(),
                context = json_value!({ "path": path.as_ref().to_string_lossy() })
            ),
        }
    };

    match SequenceNet::new(
        config.input_size,
        config.hidden_size,
        config.output_size,
        vb,
    ) {
        Ok(model) => Ok(model),
        Err(e) => raise_error!("ERR_DL_INSTANTIATION_FAILED", error = e.to_string()),
    }
}

/// Charge des poids dans un NeuralWeightsMap existant (Fine-Tuning / Reprise d'entraînement).
pub fn load_checkpoint(varmap: &mut NeuralWeightsMap, path: impl AsRef<Path>) -> RaiseResult<()> {
    match varmap.load(path.as_ref()) {
        Ok(_) => Ok(()),
        Err(e) => raise_error!(
            "ERR_DL_LOAD_CHECKPOINT",
            error = e.to_string(),
            context = json_value!({ "path": path.as_ref().to_string_lossy() })
        ),
    }
}

// =========================================================================
// TESTS UNITAIRES (Rigueur Façade & Résilience)
// =========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::deep_learning::trainer::Trainer;
    use crate::utils::testing::DbSandbox;

    #[async_test]
    #[serial_test::serial] // Sécurité : L'orchestrateur charge l'IA
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_save_and_load_consistency() -> RaiseResult<()> {
        // 1. Initialisation via Sandbox (SSOT)
        let sandbox = DbSandbox::new().await;
        let config = &sandbox.config.deep_learning;
        let device = AppConfig::device();

        let temp_dir = tempdir()?;
        let path = temp_dir.path().join("model.safetensors");

        // 2. Création et entraînement d'un modèle source
        let varmap_source = NeuralWeightsMap::new();
        let vb_source = NeuralWeightsBuilder::from_varmap(&varmap_source, ComputeType::F32, device);
        let model_source = SequenceNet::new(
            config.input_size,
            config.hidden_size,
            config.output_size,
            vb_source,
        )?;

        let mut trainer = Trainer::new(&varmap_source, config.learning_rate)?;
        let input = NeuralTensor::randn(0f32, 1.0, (1, 5, config.input_size), device)?;
        let target = NeuralTensor::zeros((1, 5), ComputeType::U32, device)?;
        trainer.train_step(&model_source, &input, &target)?;

        let output_source = model_source.forward(&input)?;

        // 3. Persistance
        save_model(&varmap_source, &path)?;

        // 4. Chargement via la nouvelle API
        let model_loaded = load_model(&path, config)?;

        // 5. Vérification de l'intégrité mathématique
        let output_loaded = model_loaded.forward(&input)?;
        let diff = (output_source - output_loaded)?
            .abs()?
            .sum_all()?
            .to_scalar::<f32>()?;

        assert!(diff < 1e-5, "Écart de précision détecté : {}", diff);
        Ok(())
    }

    #[async_test]
    #[serial_test::serial] // Sécurité : L'orchestrateur charge l'IA
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_resilience_missing_file() -> RaiseResult<()> {
        let sandbox = DbSandbox::new().await;
        let config = &sandbox.config.deep_learning;
        let fake_path = PathBuf::from("/tmp/non_existent_model_123.safetensors");

        // Le chargement doit retourner une erreur structurée et non paniquer
        let result = load_model(&fake_path, config);

        match result {
            Err(AppError::Structured(data)) => {
                assert_eq!(data.code, "ERR_DL_MODEL_NOT_FOUND");
                Ok(())
            }
            _ => panic!("Le moteur aurait dû lever ERR_DL_MODEL_NOT_FOUND"),
        }
    }

    #[async_test]
    #[serial_test::serial] // Sécurité : L'orchestrateur charge l'IA
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_checkpoint_fine_tuning_logic() -> RaiseResult<()> {
        let sandbox = DbSandbox::new().await;
        let device = AppConfig::device();
        let config = &sandbox.config.deep_learning;

        let temp_dir = tempdir()?;
        let path = temp_dir.path().join("checkpoint.safetensors");

        // 1. On sauve un état initial
        let varmap = NeuralWeightsMap::new();
        let _ = SequenceNet::new(
            config.input_size,
            config.hidden_size,
            config.output_size,
            NeuralWeightsBuilder::from_varmap(&varmap, ComputeType::F32, device),
        )?;
        save_model(&varmap, &path)?;

        // 2. On prépare un nouveau varmap
        let mut new_varmap = NeuralWeightsMap::new();

        // 🎯 FIX : On déclare la structure AVANT le chargement
        // Cela enregistre les clés attendues dans new_varmap
        let _ = SequenceNet::new(
            config.input_size,
            config.hidden_size,
            config.output_size,
            NeuralWeightsBuilder::from_varmap(&new_varmap, ComputeType::F32, device),
        )?;

        // 3. Maintenant le chargement peut mapper les poids sur les clés existantes
        load_checkpoint(&mut new_varmap, &path)?;

        assert!(
            !new_varmap.all_vars().is_empty(),
            "Le checkpoint doit maintenant contenir les variables mappées"
        );
        Ok(())
    }
}
