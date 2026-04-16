// FICHIER : src-tauri/src/utils/inference/weights.rs

use crate::utils::prelude::*;

// 🎯 On importe nos alias métier de la forteresse
use super::types::{ComputeHardware, ComputeType, NeuralWeightsBuilder};

// ⚖️ CHARGEMENT DES POIDS NEURONAUX (SafeTensors)
//
// Ce module encapsule la mécanique complexe de lecture des matrices de poids
// depuis le disque vers la mémoire vidéo (VRAM) ou système (RAM).

/// Charge les poids SafeTensors de manière sécurisée en évitant les crashs système.
pub fn load_neural_weights<'a>(
    path: &Path,
    hardware: &ComputeHardware,
) -> RaiseResult<NeuralWeightsBuilder<'a>> {
    // 1. FAIL-FAST : Vérification physique
    if !path.exists() {
        raise_error!(
            "ERR_INFERENCE_WEIGHTS_NOT_FOUND",
            error = "Le fichier de poids neuronaux (SafeTensors) est introuvable sur le disque.",
            context = json_value!({
                "requested_path": path.to_string_lossy(),
                "action": "load_neural_weights",
                "hint": "Vérifiez que le modèle (ex: Qwen2.5) a bien été téléchargé dans le dossier cible."
            })
        );
    }

    // 2. CHARGEMENT HAUTE PERFORMANCE (Memory Mapping sécurisé)
    let mmap_result = unsafe {
        NeuralWeightsBuilder::from_mmaped_safetensors(&[path], ComputeType::F32, hardware)
    };

    match mmap_result {
        Ok(builder) => Ok(builder),
        Err(e) => {
            raise_error!(
                "ERR_INFERENCE_SAFETENSORS_CORRUPTED",
                error = e,
                context = json_value!({
                    "path": path.to_string_lossy(),
                    "hardware_target": format!("{:?}", hardware),
                    "action": "memory_mapping"
                })
            );
        }
    }
}

// =========================================================================
// TESTS UNITAIRES
// =========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    fn test_load_weights_file_not_found() {
        let hardware = ComputeHardware::Cpu; // Utilisation du CPU pour garantir la portabilité du test
        let fake_path = Path::new("/chemin/totalement/fictif/model.safetensors");

        let result = load_neural_weights(fake_path, &hardware);

        match result {
            Ok(_) => panic!("Aurait dû échouer car le fichier n'existe pas."),
            Err(e) => {
                assert!(
                    e.to_string().contains("ERR_INFERENCE_WEIGHTS_NOT_FOUND"),
                    "Le Fail-Fast n'a pas remonté la bonne erreur sémantique."
                );
            }
        }
    }

    #[test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    fn test_load_weights_corrupted_file() {
        let hardware = ComputeHardware::Cpu;

        // 1. Création d'un faux fichier sur le disque (ce n'est pas un vrai SafeTensors)
        let mut temp_file = NamedTempFile::new().expect("Création du fichier temporaire échouée");
        writeln!(
            temp_file,
            "Ceci n'est absolument pas un fichier de poids valide"
        )
        .unwrap();

        // 2. Tentative de lecture via notre code unsafe encapsulé
        let path = temp_file.path();
        let result = load_neural_weights(path, &hardware);

        // 3. Vérification que l'erreur système a été proprement interceptée
        match result {
            Ok(_) => panic!("Aurait dû échouer car le fichier est corrompu/invalide."),
            Err(e) => {
                assert!(
                    e.to_string().contains("ERR_INFERENCE_SAFETENSORS_CORRUPTED"),
                    "L'erreur de memory mapping n'a pas été capturée proprement par le RaiseResult."
                );
            }
        }
    }
}
