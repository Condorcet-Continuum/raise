// FICHIER : src-tauri/src/utils/inference/hardware.rs

use crate::utils::prelude::*;

// 🎯 On importe notre alias strict depuis le fichier voisin
use super::types::ComputeHardware;

// 🧠 RÉSILIENCE MATÉRIELLE DE L'IA (Hardware Resolution)
//
// Le rôle de ce chef d'orchestre matériel est de scruter la machine physique
// et d'allouer le meilleur composant possible pour les calculs neuronaux.

/// Résout le meilleur matériel disponible (GPU prioritaire) avec un Fallback gracieux.
pub fn resolve_compute_device() -> RaiseResult<ComputeHardware> {
    #[cfg(feature = "cuda")]
    {
        match ComputeHardware::new_cuda(0) {
            Ok(device) => return Ok(device),
            Err(e) => {
                user_warn!(
                    "WRN_CUDA_UNAVAILABLE",
                    json_value!({
                        "technical_error": e.to_string(),
                        "action": "hardware_resolution",
                        "hint": "Bascule forcée sur le CPU. Les performances du moteur Neuro-Symbolique seront réduites."
                    })
                );
            }
        }
    }

    #[cfg(feature = "metal")]
    {
        match ComputeHardware::new_metal(0) {
            Ok(device) => return Ok(device),
            Err(e) => {
                user_warn!(
                    "WRN_METAL_UNAVAILABLE",
                    json_value!({
                        "technical_error": e.to_string(),
                        "hint": "Échec de l'accélération Mac. Bascule sur le processeur (CPU)."
                    })
                );
            }
        }
    }

    Ok(ComputeHardware::Cpu)
}

// =========================================================================
// TESTS UNITAIRES
// =========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_compute_device_no_panic() {
        // Le but de ce test n'est pas de vérifier l'existence d'un GPU,
        // mais de s'assurer que notre logique de Fallback fonctionne toujours
        // et qu'elle ne crashe JAMAIS, même sur un simple runner GitHub CI.
        let device_result = resolve_compute_device();

        assert!(
            device_result.is_ok(),
            "La résolution matérielle a paniqué ou échoué, violant le contrat 'Zero Deadlock'."
        );

        let device = device_result.unwrap();

        // On s'assure qu'on a bien récupéré une instance valide du type attendu
        match device {
            ComputeHardware::Cpu => {
                // C'est le comportement attendu si on n'a pas activé les features CUDA/Metal
                // ou si les drivers sont absents.
                assert!(true);
            }
            ComputeHardware::Cuda(_) | ComputeHardware::Metal(_) => {
                // C'est le comportement attendu si le test tourne sur une machine équipée.
                assert!(true);
            }
        }
    }

    #[test]
    fn test_device_is_usable() {
        use super::super::types::{ComputeType, NeuralTensor};

        // Test d'intégration ultra-léger : on vérifie que le device retourné
        // est effectivement capable de procéder à une allocation mémoire basique.
        let device = resolve_compute_device().expect("La résolution ne doit pas échouer");

        // On tente d'allouer un mini-tenseur de zéros (1x1) sur ce matériel
        let test_tensor = NeuralTensor::zeros((1, 1), ComputeType::F32, &device);

        assert!(
            test_tensor.is_ok(),
            "Le composant matériel a été résolu, mais il est incapable d'allouer de la mémoire."
        );
    }
}
