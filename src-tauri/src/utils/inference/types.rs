// FICHIER : src-tauri/src/utils/inference/types.rs

// 🧬 ALIASES SÉMANTIQUES RAISE (Forteresse des Types)
// Le reste du projet ne doit JAMAIS importer `candle_core`, `candle_nn`, `fastembed` ou `tokenizers`.
// Ces alias donnent une identité "métier" forte à l'infrastructure d'inférence.

// ============================================================================
// 1. FONDATIONS DE CALCUL (Core)
// ============================================================================

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

/// Variable mutable (tenseur dont les poids peuvent être mis à jour par l'optimiseur)
pub type NeuralVar = candle_core::Var;

/// Erreur native du moteur de calcul matriciel
pub type NeuralCoreError = candle_core::Error;

// ============================================================================
// 2. GESTION DES POIDS ET MODÈLES (NN - Weights)
// ============================================================================

/// Constructeur sécurisé pour charger et instancier les poids d'un modèle IA
pub type NeuralWeightsBuilder<'a> = candle_nn::VarBuilder<'a>;

/// Carte en mémoire stockant les paramètres variables (poids) d'un modèle
pub type NeuralWeightsMap = candle_nn::VarMap;

/// Stratégies d'initialisation des poids neuronaux (Xavier, Normal, etc.)
pub use candle_nn::init as NeuralInitStrategy;

/// Module de manipulation des poids au format SafeTensors
pub use candle_core::safetensors as SafeTensorsIO;

/// Module de manipulation des poids quantifiés au format GGUF
pub use candle_core::quantized::gguf_file as GgufFileFormat;

// ============================================================================
// 3. ARCHITECTURE ET COUCHES NEURONALES (Layers & Traits)
// ============================================================================

/// Le trait qui définit une passe avant (Forward Pass) pour un module neuronal
pub use candle_core::Module as NeuralModule;

/// Le trait spécifique aux réseaux récurrents (permettant la fonction .step())
pub use candle_nn::rnn::RNN as NeuralRnnTrait;

/// Une couche de réseau de neurones dense (Fully Connected)
pub type NeuralLinearLayer = candle_nn::Linear;

/// Une couche de réseau de neurones récurrent avec mémoire à long/court terme (LSTM)
pub type NeuralLstmLayer = candle_nn::rnn::LSTM;

/// Couche de plongement (Embedding) pour transformer des identifiants discrets en vecteurs continus
pub type NeuralEmbeddingLayer = candle_nn::Embedding;

/// Fonctions d'activation mathématiques (ReLU, GeLU, Swish, etc.)
pub type NeuralActivation = candle_nn::Activation;

pub use candle_nn::embedding as init_embedding_layer;
pub use candle_nn::linear as init_linear_layer;
pub use candle_nn::rnn::lstm as init_lstm_layer;

// ============================================================================
// 4. APPRENTISSAGE ET OPTIMISATION (Training)
// ============================================================================

pub use candle_nn::optim::Optimizer as NeuralOptimizerTrait;
pub type NeuralOptimizerAdamW = candle_nn::optim::AdamW;
pub type OptimizerConfigAdamW = candle_nn::optim::ParamsAdamW;
pub use candle_nn::loss::cross_entropy as compute_cross_entropy;

// ============================================================================
// 5. TOKENISATION ET TRAITEMENT DU TEXTE (NLP)
// ============================================================================

/// Moteur de tokenisation (convertit le texte en identifiants numériques pour l'IA)
pub type TextTokenizer = tokenizers::Tokenizer;

// ============================================================================
// 5.5. VECTORISATION DE TEXTE & NLP NATIF (BERT)
// ============================================================================

/// Architecture du modèle BERT utilisé pour générer des embeddings (RAG)
pub use candle_transformers::models::bert::BertModel as NeuralBertModel;

/// Configuration structurelle spécifique aux modèles de type BERT
pub use candle_transformers::models::bert::Config as NeuralBertConfig;

// ============================================================================
// 6. GÉNÉRATION TEXTUELLE & LLM (Transformers)
// ============================================================================

/// Processeur stochastique qui gère la température, Top-P, Top-K lors de la génération
pub type TokenLogitsProcessor = candle_transformers::generation::LogitsProcessor;

/// Module contenant l'architecture du modèle LLM Qwen2 (version quantifiée)
pub use candle_transformers::models::quantized_qwen2 as Qwen2QuantizedModel;

// ============================================================================
// 7. MULTIMODALITÉ : AUDIO & VISION (Whisper)
// ============================================================================

pub use candle_transformers::models::whisper as WhisperModel;
pub use candle_transformers::models::whisper::audio as WhisperAudio;
pub type WhisperConfig = candle_transformers::models::whisper::Config;

// ============================================================================
// 8. MOTEUR D'EMBEDDINGS LÉGER (ONNX / CPU)
// ============================================================================

/// Moteur d'inférence léger pour la génération vectorielle (pure CPU/ONNX)
pub type LightweightTextEmbedding = fastembed::TextEmbedding;

/// Énumération des architectures de modèles légers supportées (BGE, MiniLM, etc.)
pub use fastembed::EmbeddingModel as LightweightEmbeddingModel;

/// Options de configuration pour l'initialisation du moteur léger
pub type LightweightInitOptions = fastembed::InitOptions;
