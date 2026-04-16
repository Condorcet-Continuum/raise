// FICHIER : src-tauri/src/utils/inference/embeddings.rs

use crate::utils::prelude::*;

/// Modèle de vectorisation par défaut (Optimisé pour tourner sur une VRAM modeste)
pub const DEFAULT_EMBED_MODEL: &str = "BAAI/bge-small-en-v1.5";

/// 🧠 MOTEUR D'EMBEDDING (Text-to-Vector)
///
/// Cette forteresse isole la librairie tierce `fastembed`. Elle permet de
/// transformer du texte en vecteurs pour notre graphe de connaissances sémantique.
/// Si demain on décide d'utiliser une API distante ou un autre modèle ONNX,
/// seule l'implémentation de cette structure changera.
pub struct TextEmbedder {
    // L'implémentation sous-jacente est totalement masquée au reste du code.
    inner: fastembed::TextEmbedding,
}

impl TextEmbedder {
    /// Initialise le moteur de vectorisation de manière sécurisée.
    pub fn new() -> RaiseResult<Self> {
        // Configuration d'un modèle léger et ultra-rapide
        let opts = fastembed::InitOptions::new(fastembed::EmbeddingModel::BGESmallENV15);

        match fastembed::TextEmbedding::try_new(opts) {
            Ok(inner) => Ok(Self { inner }),
            Err(e) => {
                // Fail-Fast propre : on capture l'erreur si le modèle n'a pas
                // pu être chargé (ex: pas d'espace disque, erreur réseau au 1er lancement).
                raise_error!(
                    "ERR_INFERENCE_EMBEDDER_INIT",
                    error = e,
                    context = json_value!({
                        "model": DEFAULT_EMBED_MODEL,
                        "action": "init_fastembed",
                        "hint": "Vérifiez l'espace disque pour le modèle d'embeddings."
                    })
                );
            }
        }
    }

    /// Transforme un lot (batch) de textes en vecteurs denses.
    /// Cette opération est encapsulée pour capturer toute erreur OOM (Out Of Memory).
    pub fn embed_batch(&mut self, texts: Vec<&str>) -> RaiseResult<Vec<Vec<f32>>> {
        let batch_size = texts.len();

        match self.inner.embed(texts, None) {
            Ok(embeddings) => Ok(embeddings),
            Err(e) => {
                raise_error!(
                    "ERR_INFERENCE_EMBEDDING_FAIL",
                    error = e,
                    context = json_value!({
                        "batch_size": batch_size,
                        "action": "generate_embeddings"
                    })
                );
            }
        }
    }
}

// =========================================================================
// TESTS UNITAIRES
// =========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    fn test_embedder_initialization() {
        // Vérifie que le constructeur ne panique pas et charge bien le modèle
        let result = TextEmbedder::new();
        assert!(
            result.is_ok(),
            "L'initialisation de FastEmbed a échoué. Cause: {:?}",
            result.err()
        );
    }

    #[test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    fn test_single_embedding_dimension() {
        let mut embedder = TextEmbedder::new().expect("Le modèle devrait s'initialiser");
        let texts = vec!["L'architecture RAISE garantit le Zéro Dette."];

        let result = embedder
            .embed_batch(texts)
            .expect("La vectorisation a échoué");

        // 1. On vérifie qu'un seul vecteur est retourné
        assert_eq!(result.len(), 1);

        // 2. On vérifie la taille stricte du vecteur (384 pour BGE-Small)
        assert_eq!(
            result[0].len(),
            384,
            "La dimension du vecteur ne correspond pas au modèle BGE-Small"
        );
    }

    #[test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    fn test_batch_embeddings_processing() {
        let mut embedder = TextEmbedder::new().expect("Le modèle devrait s'initialiser");

        let texts = vec![
            "Court.",
            "Une phrase de taille moyenne pour tester l'analyse sémantique.",
            "Voici un texte beaucoup plus long qui représente le contenu d'un document SysML complet, avec des spécifications techniques, des contraintes de performance et des exigences de traçabilité strictes."
        ];

        let result = embedder
            .embed_batch(texts.clone())
            .expect("La vectorisation par lot a échoué");

        assert_eq!(
            result.len(),
            texts.len(),
            "Le nombre de vecteurs retournés ne correspond pas au batch initial"
        );

        for (i, embedding) in result.iter().enumerate() {
            assert_eq!(
                embedding.len(),
                384,
                "Le vecteur à l'index {} a une dimension invalide",
                i
            );

            let sum: f32 = embedding.iter().map(|v| v.abs()).sum();
            assert!(
                sum > 0.0,
                "Le vecteur à l'index {} est vide (somme nulle)",
                i
            );
        }
    }
}
