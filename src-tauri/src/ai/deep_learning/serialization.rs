use crate::ai::deep_learning::models::sequence_net::SequenceNet;
use crate::utils::io::Path;
use candle_core::{DType, Device, Result};
use candle_nn::{VarBuilder, VarMap};

/// Sauvegarde les poids du modèle dans un fichier au format SafeTensors.
///
/// # Arguments
/// * `varmap` - Le conteneur de variables (poids) à sauvegarder.
/// * `path` - Le chemin de destination (ex: "model.safetensors").
pub fn save_model(varmap: &VarMap, path: impl AsRef<Path>) -> Result<()> {
    varmap.save(path)
}

/// Charge un modèle SequenceNet complet depuis un fichier pour l'inférence.
///
/// # Arguments
/// * `path` - Chemin du fichier .safetensors.
/// * `input_size`, `hidden_size`, `output_size` - Hyperparamètres de l'architecture.
/// * `device` - Périphérique sur lequel charger le modèle (CPU/CUDA).
pub fn load_model(
    path: impl AsRef<Path>,
    input_size: usize,
    hidden_size: usize,
    output_size: usize,
    device: &Device,
) -> Result<SequenceNet> {
    // Chargement via mmap pour la rapidité (bloc unsafe requis par candle pour mmap)
    let vb = unsafe { VarBuilder::from_mmaped_safetensors(&[path], DType::F32, device)? };

    // Reconstruction du modèle avec les poids chargés
    SequenceNet::new(input_size, hidden_size, output_size, vb)
}

/// Charge des poids dans un VarMap existant (utile pour le Fine-Tuning ou reprendre un entraînement).
pub fn load_checkpoint(varmap: &mut VarMap, path: impl AsRef<Path>) -> Result<()> {
    varmap.load(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::deep_learning::trainer::Trainer;
    use crate::utils::io::{self, Path};
    use candle_core::Tensor;

    #[tokio::test]
    async fn test_save_and_load_consistency() -> Result<()> {
        let device = Device::Cpu;
        let path = "/tmp/test_raise_model.safetensors";

        // 1. Création et modification d'un modèle "Source"
        let varmap_source = VarMap::new();
        let vb_source = VarBuilder::from_varmap(&varmap_source, DType::F32, &device);
        let model_source = SequenceNet::new(10, 20, 5, vb_source)?;

        // On fait un pas d'entraînement pour modifier les poids initiaux
        let trainer = Trainer::new(&varmap_source, 0.1);
        let input = Tensor::randn(0f32, 1.0, (1, 5, 10), &device)?;
        let target = Tensor::zeros((1, 5), DType::U32, &device)?;
        trainer.train_step(&model_source, &input, &target)?;

        // Prédiction témoin avant sauvegarde
        let output_source = model_source.forward(&input)?;

        // 2. Sauvegarde
        save_model(&varmap_source, path)?;
        assert!(Path::new(path).exists());

        // 3. Chargement dans un nouveau modèle "Target"
        // Note: On n'utilise pas de VarMap ici, le VarBuilder est créé directement depuis le fichier
        let model_loaded = load_model(path, 10, 20, 5, &device)?;

        // 4. Vérification : Les prédictions doivent être IDENTIQUES
        let output_loaded = model_loaded.forward(&input)?;

        let diff = (output_source - output_loaded)?
            .abs()?
            .sum_all()?
            .to_scalar::<f32>()?;

        // Nettoyage
        let _ = io::remove_file(Path::new(path)).await;

        assert!(
            diff < 1e-5,
            "Le modèle chargé diffère du modèle original (diff: {})",
            diff
        );
        Ok(())
    }
}
