// FICHIER : src-tauri/src/utils/prelude.rs

// =========================================================================
//  RAISE PRELUDE - L'Unique Façade Sémantique (Zéro Dette)
// =========================================================================

// --- 1. CORE, ERREURS & FONDATIONS (Synchronisés avec core/mod.rs) ---
pub use crate::utils::context::i18n::I18nString;
pub use crate::utils::core::error::{anyhow, AnyResult, AppError, Context, RaiseResult};
pub use crate::utils::core::{
    async_interface, // 🎯 Alias de async_trait::async_trait
    async_recursive,
    async_test,
    is_same_reference,
    memory_copy_fast,
    sleep_async,
    // Runtime & Tasks
    spawn_async_task,
    spawn_cpu_task,
    terminate_process,
    AgentAttention,
    AsyncChannel,
    AsyncCommand,
    AsyncFuture,
    AsyncMutex,
    AsyncRwLock,
    BufferedRead,
    CalendarDate,
    CalendarDuration,
    CowData,
    DataStreamPeekable, // 🎯 Pour l'anticipation (lookahead)
    Eq,
    // Formatage Sémantique
    FmtCursor, // 🎯 Remplace Formatter (plus visuel: on écrit là où est le curseur)
    FmtDebug,
    FmtDisplay,  // 🎯 Remplace Display (plus explicite pour l'IA)
    FmtOrdering, // 🎯 L'alias sémantique pour Ordering
    FmtResult,   // 🎯 Résultat de l'opération de formatage
    Hashable,
    InitGuard,      // 🎯 Alias de Once
    LocalClock,     // 🎯 Alias de chrono::Local
    LocalTimestamp, // 🎯 Alias de chrono::DateTime<Local>
    MaxOf,
    MemoryCache,
    MinOf,
    Ord,
    Parsable,
    PartialEq,
    PartialOrd,
    Pinned,
    SafeSize,
    // Concurrence & Mémoire (Alias RAISE)
    SharedRef,  // 🎯 Alias de Arc
    StaticCell, // 🎯 Alias de OnceLock
    SyncMutex,
    SyncRwLock,
    SystemStr, // 🎯 Pour la compatibilité OS native
    TextChars, // 🎯 Pour le découpage atomique du texte
    TextRegex,
    TextRegexError,
    TimeDuration, // 🎯 Alias de std::time::Duration
    TimeInstant,
    TypeMarker,
    // Identifiants & Temps (Alias RAISE)
    UniqueId,     // 🎯 Alias de uuid::Uuid
    UtcClock,     // 🎯 Alias de chrono::Utc
    UtcTimestamp, // 🎯 Alias de chrono::DateTime<Utc>
    MATH_PI,      // 🎯 La constante fondamentale
};

// --- 2. I/O, FS & SYSTÈME ---
pub use crate::utils::io::io_traits::{SyncBufRead, SyncRead, SyncSeek, SyncWrite};
pub use crate::utils::io::os_types::{ProcessCommand, ProcessIoConfig, ProcessOutput};
pub use crate::utils::io::{
    compress, decompress, fs, os, stderr_raw, stdin_raw, stdout_raw, tempdir, Path, PathBuf,
    TempDir,
};

// --- 3. DATA, JSON & COLLECTIONS ---
pub use crate::utils::data::compute::{execute_compute_plan, ComputeOperatorFn, COMPUTE_REGISTRY};
pub use crate::utils::data::config::{
    AppConfig,
    CoreConfig,
    DeepLearningConfig, // 🎯 L'élément manquant pour ton agent DL
    WorldModelConfig,
};
pub use crate::utils::data::json::{self, json_value, JsonObject, JsonValue};
pub use crate::utils::data::{
    Deserializable,
    DeserializableOwned,
    OrderedMap, // 🎯 BTreeMap sémantique
    Serializable,
    UniqueSet,    // 🎯 HashSet sémantique
    UnorderedMap, // 🎯 HashMap sémantique
};

// --- 5. INFÉRENCE & MACHINE LEARNING (Forteresse RAISE) ---
// Ces exports masquent totalement l'écosystème ML (Candle, FastEmbed, Rayon)
// au reste du projet pour garantir un couplage faible et une haute résilience.
pub use crate::utils::inference::{
    configure_parallel_pool, // Limiteur de ressources pour éviter d'étouffer l'OS
    // ⚡ 5. Parallélisme CPU (Multi-threading)
    execute_parallel_map, // Itération parallèle ultra-rapide et thread-safe
    load_neural_weights,  // Charge les fichiers .safetensors sans risque de crash (mmap)

    // 🛡️ 3. Fonctions d'Initialisation Sécurisées (Fail-Fast)
    resolve_compute_device, // Alloue le meilleur GPU disponible avec fallback CPU
    ComputeHardware,        // Matériel physique cible (CUDA, Metal, CPU)
    // 🧬 1. Types Fondamentaux (Calcul Matriciel)
    ComputeType, // Précision mathématique requise (ex: F32, F16)
    DimIndex,    // Outil d'indexation pour manipuler les dimensions (D)

    NeuralShape,  // Dimensions des matrices/tenseurs
    NeuralTensor, // Structure de données cœur pour les calculs d'IA
    // ⚖️ 2. Gestion des Modèles et Poids (NN)
    NeuralWeightsBuilder, // Constructeur pour charger les paramètres d'un modèle
    NeuralWeightsMap,     // Espace mémoire hébergeant les poids du réseau

    // 🧠 4. Moteurs Sémantiques
    TextEmbedder, // Générateur de vecteurs pour la recherche RAG
};

// --- 4. RÉSEAU & CONNECTIVITÉ ---
pub use crate::utils::network::http_types::{
    run_http_server, HttpClient, HttpClientBuilder, HttpJsonPayload, HttpRouter, HttpStatusCode,
};

pub use crate::utils::network::p2p_types::{
    P2pConnectionLimits, P2pGossipSub, P2pIdentity, P2pKademlia, P2pMultiaddr, P2pPeerId, P2pSwarm,
};

pub use crate::utils::network::{
    get_client, get_string_async, post_authenticated_async, post_json_with_retry_async,
    start_local_api_async,
};

// --- 5. MACROS & OBSERVABILITÉ (Exports Racine) ---
pub use crate::{
    build_error, raise_error, require_session, user_debug, user_error, user_info, user_success,
    user_trace, user_warn,
};

// On expose les logs de core pour le paramétrage du moteur
pub use crate::utils::core::logs;
