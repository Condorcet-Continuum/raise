// FICHIER : src-tauri/src/utils/prelude.rs

// =========================================================================
//  RAISE PRELUDE - L'Unique Façade Sémantique (Zéro Dette)
// =========================================================================

// --- 1. CORE, ERREURS & FONDATIONS (Synchronisés avec core/mod.rs) ---
pub use crate::utils::core::error::{anyhow, AnyResult, AppError, Context, RaiseResult};
pub use crate::utils::core::{
    async_interface, // 🎯 Alias de async_trait::async_trait
    async_recursive,
    async_test,        // 🎯 Alias de tokio::test
    is_same_reference, // 🎯 Alias de ptr::eq
    sleep_async,
    // Runtime & Tasks
    spawn_async_task,
    spawn_cpu_task,
    AsyncChannel,
    AsyncCommand,
    AsyncFuture,
    AsyncMutex,
    AsyncRwLock,
    BufferedRead,
    CalendarDate,
    CalendarDuration,
    CowData,
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
    MemoryCache,
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
    TextRegex,
    TextRegexError,
    TimeDuration, // 🎯 Alias de std::time::Duration
    TimeInstant,
    TypeMarker,
    // Identifiants & Temps (Alias RAISE)
    UniqueId,     // 🎯 Alias de uuid::Uuid
    UtcClock,     // 🎯 Alias de chrono::Utc
    UtcTimestamp, // 🎯 Alias de chrono::DateTime<Utc>
};

// --- 2. I/O, FS & SYSTÈME ---
pub use crate::utils::io::io_traits::{SyncBufRead, SyncRead, SyncSeek, SyncWrite};
pub use crate::utils::io::os_types::{ProcessCommand, ProcessIoConfig, ProcessOutput};
pub use crate::utils::io::{
    compress, decompress, fs, os, stderr_raw, stdin_raw, stdout_raw, tempdir, Path, PathBuf,
    TempDir,
};

// --- 3. DATA, JSON & COLLECTIONS ---
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
    user_warn,
};

// On expose les logs de core pour le paramétrage du moteur
pub use crate::utils::core::logs;
