use crate::utils::prelude::*;

use crate::ai::deep_learning::models::sequence_net::SequenceNet;
use candle_core::DType;
use candle_nn::{VarBuilder, VarMap};

/// Sauvegarde les poids du modèle dans un fichier au format SafeTensors.
///
/// # Arguments
/// * `varmap` - Le conteneur de variables (poids) à sauvegarder.
/// * `path` - Le chemin de destination (ex: "model.safetensors").
pub fn save_model(varmap: &VarMap, path: impl AsRef<Path>) -> RaiseResult<()> {
    varmap.save(path)?;
    Ok(())
}

/// Charge un modèle SequenceNet complet depuis un fichier pour l'inférence.
///
/// # Arguments
/// * `path` - Chemin du fichier .safetensors.
/// * `input_size`, `hidden_size`, `output_size` - Hyperparamètres de l'architecture.
/// * `device` - Périphérique sur lequel charger le modèle (CPU/CUDA).
pub fn load_model(path: impl AsRef<Path>, config: &DeepLearningConfig) -> RaiseResult<SequenceNet> {
    let device = config.to_device();
    let vb = unsafe { VarBuilder::from_mmaped_safetensors(&[path], DType::F32, &device)? };
    SequenceNet::new(
        config.input_size,
        config.hidden_size,
        config.output_size,
        vb,
    )
}

/// Charge des poids dans un VarMap existant (utile pour le Fine-Tuning ou reprendre un entraînement).
pub fn load_checkpoint(varmap: &mut VarMap, path: impl AsRef<Path>) -> RaiseResult<()> {
    varmap.load(path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::deep_learning::trainer::Trainer;
    use crate::utils::testing::DbSandbox;
    use candle_core::{DType, Tensor};

    #[async_test]
    async fn test_save_and_load_consistency() -> RaiseResult<()> {
        // 1. Initialisation du Singleton avec le "moule de test" (10, 20, 5, LR=0.1)
        let sandbox = DbSandbox::new().await;
        let config = &sandbox.config.deep_learning;
        let device = config.to_device();
        let path = "/tmp/test_raise_model.safetensors";

        // 2. Création du modèle Source avec les dimensions de la config (SSOT)
        let varmap_source = VarMap::new();
        let vb_source = VarBuilder::from_varmap(&varmap_source, DType::F32, &device);
        let model_source = SequenceNet::new(
            config.input_size,
            config.hidden_size,
            config.output_size,
            vb_source,
        )?;

        // Entraînement rapide pour modifier les poids
        let mut trainer = Trainer::new(&varmap_source, config.learning_rate)?; // 🎯 N'oublie pas `mut` et `?`
        let input = Tensor::randn(0f32, 1.0, (1, 5, config.input_size), &device)?;
        let target = Tensor::zeros((1, 5), DType::U32, &device)?;
        trainer.train_step(&model_source, &input, &target)?;

        let output_source = model_source.forward(&input)?;

        // 3. Sauvegarde
        save_model(&varmap_source, path)?;

        // 🎯 4. CHARGEMENT CORRIGÉ (2 arguments au lieu de 5)
        // On passe simplement l'objet config qui contient déjà input/hidden/output/device
        let model_loaded = load_model(path, config)?;

        // 5. Vérification
        let output_loaded = model_loaded.forward(&input)?;
        let diff = (output_source - output_loaded)?
            .abs()?
            .sum_all()?
            .to_scalar::<f32>()?;

        let _ = fs::remove_file_async(Path::new(path)).await;

        assert!(diff < 1e-5, "Le modèle chargé diffère (diff: {})", diff);
        Ok(())
    }
}
