// FICHIER : src-tauri/src/utils/inference/mod.rs

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
// =========================================================================
// 1. DÉCLARATION DES MODULES THÉMATIQUES
// =========================================================================
// Ces fichiers contiennent l'implémentation physique et les dépendances.
pub mod embeddings; // Génération de vecteurs sémantiques
pub mod hardware; // Détection et gestion du GPU/CPU
pub mod parallel; // Abstraction du multi-threading CPU
pub mod types; // Alias stricts (NeuralTensor, ComputeHardware, etc.)
pub mod weights; // Chargement sécurisé des fichiers .safetensors

// =========================================================================
// 2. RÉ-EXPORTS SÉMANTIQUES (La Façade RAISE)
// =========================================================================

// --- Types et Fondations (L'Arsenal Sémantique) ---
// Nous exportons nos alias personnalisés pour qu'ils soient accessibles
// globalement via `crate::utils::prelude::*`.
pub use types::{
    compute_cross_entropy,

    init_embedding_layer,

    init_linear_layer,
    init_lstm_layer,
    // 1. Fondations de calcul
    ComputeHardware,
    ComputeType,
    DimIndex,
    GgufFileFormat, // I/O de modèles quantifiés

    LightweightEmbeddingModel, // Modèles CPU (BGE, MiniLM)
    LightweightInitOptions,    // Paramètres d'exécution légers
    // 8. Moteur d'Embeddings Léger (ONNX / CPU)
    LightweightTextEmbedding, // FastEmbed natif
    NeuralActivation,         // Fonctions d'activation mathématiques
    NeuralBertConfig,         // Configuration du modèle BERT

    NeuralBertModel, // Moteur natif de vectorisation (BERT)
    NeuralCoreError, // Erreur native du moteur Tensoriel

    NeuralEmbeddingLayer, // Couche de plongement
    NeuralInitStrategy,   // Stratégies d'initialisation
    NeuralLinearLayer,
    NeuralLstmLayer,
    // 3. Architecture et Couches Neuronales
    NeuralModule,
    NeuralOptimizerAdamW,
    // 4. Entraînement et Optimisation
    NeuralOptimizerTrait,
    NeuralRnnTrait,
    NeuralShape,
    NeuralTensor,
    NeuralVar, // Variable mutable pour l'optimiseur
    // 2. Gestion des Poids et Modèles
    NeuralWeightsBuilder,
    NeuralWeightsMap,
    OptimizerConfigAdamW,
    Qwen2QuantizedModel, // Moteur LLM natif

    SafeTensorsIO, // I/O de poids natifs
    // 5. Tokenisation et Traitement du Texte (NLP)
    TextTokenizer, // Moteur de tokenisation
    // 6. Génération Textuelle & LLM (Transformers)
    TokenLogitsProcessor, // Gestionnaire de température / Top-P
    WhisperAudio,         // Traitement du signal (Mel, MFCC)
    WhisperConfig,        // Paramétrage audio

    // 7. Multimodalité : Audio & Vision (Whisper)
    WhisperModel, // Architecture de transcription
};

// --- Matériel et Poids ---
// Fonctions critiques de démarrage et d'allocation de ressources.
pub use hardware::{resolve_compute_device, NvidiaMonitor};
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
// directement les crates 'candle', 'rayon', 'fastembed' ou 'tokenizers'.
// Utilisez toujours le prélude : `use crate::utils::prelude::*;`
