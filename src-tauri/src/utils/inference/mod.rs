// FICHIER : src-tauri/src/utils/inference/mod.rs

pub mod embeddings; // Génération de vecteurs sémantiques
pub mod hardware; // Détection et gestion du GPU/CPU
pub mod parallel;
/// 🚀 MODULE INFÉRENCE - LA FORTERESSE NEURO-SYMBOLIQUE
///
/// Ce module centralise et sécurise toutes les capacités d'intelligence
/// artificielle du projet. Il isole les dépendances lourdes (Candle, Rayon, FastEmbed)
/// derrière une interface sémantique stable et robuste.
///
/// Architecture "Zéro Dette" :
/// 1. Les types tiers sont aliasés dans `types` pour un couplage faible.
/// 2. Le matériel est résolu de manière résiliente dans `hardware`.
/// 3. Les opérations système (I/O, Threads) sont encapsulées et monitorées.
// --- 1. DÉCLARATION DES MODULES THÉMATIQUES ---
// Ces fichiers contiennent l'implémentation physique et les dépendances.
pub mod types; // Alias stricts (NeuralTensor, ComputeHardware, etc.)
pub mod weights; // Chargement sécurisé des fichiers .safetensors // Abstraction du multi-threading CPU

// =========================================================================
// 2. RÉ-EXPORTS SÉMANTIQUES (La Façade RAISE)
// =========================================================================

// --- Types et Fondations ---
// Nous exportons nos alias personnalisés pour qu'ils soient accessibles
// via `crate::utils::prelude::*`.
pub use types::{
    ComputeHardware,      // Cible (Cuda, Metal, Cpu)
    ComputeType,          // Précision (F32, F16...)
    DimIndex,             // Indexeur de dimensions (D)
    NeuralShape,          // Dimensions des matrices
    NeuralTensor,         // Structure de calcul fondamentale
    NeuralWeightsBuilder, // Chargeur de poids
    NeuralWeightsMap,     // Registre de paramètres
};

// --- Matériel et Poids ---
// Fonctions critiques de démarrage et d'allocation de ressources.
pub use hardware::resolve_compute_device;
pub use weights::load_neural_weights;

// --- Capacités d'Analyse ---
// Moteur de vectorisation et constantes de configuration.
pub use embeddings::{TextEmbedder, DEFAULT_EMBED_MODEL};

// --- Performance et Parallélisme ---
// Outils de calcul intensif sur le processeur.
pub use parallel::{configure_parallel_pool, execute_parallel_map};

// =========================================================================
// 3. DOCUMENTATION ARCHITECTURALE
// =========================================================================
// 🤖 NOTE POUR L'IA :
// Pour utiliser l'inférence dans un nouvel agent, ne jamais importer
// directement les crates 'candle' ou 'rayon'.
// Utilisez toujours le prélude : `use crate::utils::prelude::*;`
