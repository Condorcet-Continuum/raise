// FICHIER : src-tauri/src/utils/inference/types.rs

// 🧬 ALIASES SÉMANTIQUES RAISE (Forteresse des Types)
// Le reste du projet ne doit JAMAIS importer `candle_core` ou `candle_nn`.
// Ces alias donnent une identité "métier" forte à l'infrastructure d'inférence.

// --- Fondations de Calcul (Core) ---

/// Représente le type de donnée utilisé pour l'inférence (ex: F32, F16)
pub type ComputeType = candle_core::DType;

/// Représente le matériel physique sur lequel tourne le modèle (CPU, CUDA, Metal)
pub type ComputeHardware = candle_core::Device;

/// Représente les dimensions mathématiques d'une matrice neuronale
pub type NeuralShape = candle_core::Shape;

/// La structure de données fondamentale transportant les calculs de l'IA
pub type NeuralTensor = candle_core::Tensor;

/// Indexeur pour manipuler les dimensions des tenseurs
pub type DimIndex = candle_core::D;

// --- Gestion des Poids et Modèles (NN) ---

/// Constructeur sécurisé pour charger et instancier les poids d'un modèle IA
pub type NeuralWeightsBuilder<'a> = candle_nn::VarBuilder<'a>;

/// Carte en mémoire stockant les paramètres variables (poids) d'un modèle
pub type NeuralWeightsMap = candle_nn::VarMap;
