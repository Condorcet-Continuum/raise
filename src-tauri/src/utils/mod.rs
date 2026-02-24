// FICHIER : src-tauri/src/utils/mod.rs

// =========================================================================
//  RAISE UTILS V1.0 - Foundation Layer (Stable)
// =========================================================================

// --- 1. MODULES INTERNES ---
pub mod compression;
pub mod config;
pub mod error;
pub mod fs;
pub mod i18n;
pub mod json;
pub mod logger;
pub mod macros;
pub mod net;
pub mod os;

// --- 2. FAÇADES SÉMANTIQUES (L'Architecture Cible) ---

/// **Core Foundation** : Types de base et Erreurs.
pub mod core {
    // [AJOUT SILENCIEUX] On ajoute RaiseResult à côté de Result
    pub use super::error::{AppError, RaiseResult, Result};
    pub use chrono::{DateTime, Utc};
    pub use uuid::Uuid;
}

/// **System Operations**
pub mod sys {
    pub use super::os::{exec_command, pipe_through};
}

/// **Physical Layer (I/O)** : Accès disque sécurisé (Atomicité + Sandboxing).
pub mod io {
    pub use super::compression::{compress, decompress};
    pub use super::fs::{
        copy, copy_dir_all, create_dir_all, ensure_dir, exists, include_dir, read,
        read_bincode_compressed, read_compressed, read_dir, read_json, read_json_compressed,
        read_to_string, remove_dir_all, remove_file, rename, tempdir, write, write_atomic,
        write_bincode_compressed_atomic, write_compressed_atomic, write_json_atomic,
        write_json_compressed_atomic, Component, Dir, DirEntry, File, Path, PathBuf, ProjectScope,
        TempDir,
    };
}

/// **Data Abstraction** : Manipulation JSON et Contextes.
pub mod data {
    pub use super::json::{
        from_binary, from_value, json, merge, parse, stringify, stringify_pretty, to_binary,
        to_value, to_vec, ContextBuilder, Map, Value,
    };
    pub use serde::{Deserialize, Serialize};
    pub use std::collections::{BTreeMap, HashMap, HashSet};
}

/// **Application Context** : Accès global Config/Log/Env.
pub mod context {
    pub use super::config::AppConfig;
    pub use super::i18n::{init_i18n, t};
    pub use super::logger::init_logging;
}

/// **Connectivity** : Clients HTTP robustes.
pub mod net_client {
    pub use super::net::{get_client, get_simple, post_authenticated, post_json_with_retry};
}

/// **Le Prélude** : À utiliser via `use crate::utils::prelude::*;`
pub mod prelude {
    pub use super::context::AppConfig;
    // [AJOUT SILENCIEUX] On glisse RaiseResult ici aussi
    pub use super::core::{AppError, RaiseResult, Result, Utc, Uuid};
    pub use super::data::{json, Deserialize, Serialize, Value};
    pub use super::io::Path;
    pub use crate::{user_error, user_info, user_success};
    pub use tracing::{debug, error, info, instrument, warn};
}

// =========================================================================
// 3. EXPORTS LEGACY & UTILITAIRES (Compatibilité Totale)
// =========================================================================

// --> Config & Erreurs
pub use config::AppConfig;
// [AJOUT SILENCIEUX] Export legacy mis à jour pour inclure RaiseResult
pub use error::{AppError, RaiseResult, Result};
pub use logger::init_logging;

// --> Domaine (Requis par migrator.rs et autres)
pub use chrono::{DateTime, Utc};
pub use std::str::FromStr;
pub use uuid::Uuid;

// --> Logging (Requis par manager.rs)
pub use tracing::{debug, error, info, instrument, warn};

// --> Async Runtime & Sync
pub use std::future::Future;
pub use std::pin::Pin;
pub use std::sync::{
    Arc, Mutex, MutexGuard, Once, OnceLock, RwLock, RwLockReadGuard, RwLockWriteGuard,
};
pub use tokio::sync::mpsc;
pub use tokio::sync::Mutex as AsyncMutex;
pub use tokio::sync::RwLock as AsyncRwLock;
pub use tokio::time::sleep;

// --> Macros externes
pub use async_recursion::async_recursion;
pub use async_trait::async_trait;

// --> I/O
pub use std::io::{BufRead, Read, Seek, Write};
pub use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
pub use tokio::process;

// --> Collections & Types
pub use lru::LruCache;
pub use std::cmp::Ordering;
pub use std::collections::{BTreeMap, HashMap, HashSet};
pub use std::fmt;
pub use std::hash::Hash;
pub use std::num::NonZeroUsize;
pub use std::thread;
pub use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

// --> Regex
pub use regex::Regex;
