// FICHIER : src-tauri/src/utils/mod.rs

// =========================================================================
//  RAISE UTILS V1.0 - Foundation Layer (Stable)
// =========================================================================

// --- 1. MODULES INTERNES ---
// Nous gardons ces modules publics et non-deprecated pour l'instant
// afin de ne pas casser le build (Clippy). La migration se fera progressivement.

pub mod compression;
pub mod config;
pub mod env;
pub mod error;
pub mod fs;
pub mod i18n;
pub mod json;
pub mod logger;
pub mod macros;
pub mod net;
pub mod os;

// --- 2. FAÇADES SÉMANTIQUES (L'Architecture Cible) ---
// Ce sont les points d'entrée que le nouveau code (ex: Code Generator) DOIT utiliser.

/// **Core Foundation** : Types de base et Erreurs.
pub mod core {
    pub use super::error::{AppError, Result};
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
        copy_dir_all, create_dir_all, ensure_dir, exists, include_dir, read_compressed, read_json,
        read_json_compressed, read_to_string, remove_dir_all, remove_file, rename, tempdir,
        write_atomic, write_compressed_atomic, write_json_atomic, write_json_compressed_atomic,
        Dir, DirEntry, Path, PathBuf, ProjectScope, TempDir,
    };
}
/// **Data Abstraction** : Manipulation JSON et Contextes.
pub mod data {
    pub use super::json::{
        // Parsing & Conversion
        from_value,
        // Types
        json,
        // Fusion & Construction
        merge,
        parse,
        stringify,
        stringify_pretty,
        to_value,
        ContextBuilder,
        Map,
        Value,
    };
    pub use serde::{Deserialize, Serialize};
    pub use std::collections::{HashMap, HashSet};
}

/// **Application Context** : Accès global Config/Log/Env.
pub mod context {
    pub use super::config::AppConfig;
    pub use super::env::{get, get_or, is_enabled};
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
    pub use super::core::{AppError, Result, Utc, Uuid};
    pub use super::data::{json, Deserialize, Serialize, Value};
    pub use tracing::{debug, error, info, instrument, warn};
}

// =========================================================================
// 3. EXPORTS LEGACY & UTILITAIRES (Compatibilité Totale)
// =========================================================================
// Ces exports sont requis par le code existant (json_db, commands, etc.)
// Ne rien supprimer ici tant que tout le projet n'est pas migré.

// --> Config & Erreurs
pub use config::AppConfig;
pub use error::{AppError, Result};
pub use logger::init_logging;

// --> Domaine (Requis par migrator.rs et autres)
pub use chrono::{DateTime, Utc};
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
pub use tokio::sync::RwLock as AsyncRwLock;
pub use tokio::time::sleep;

// --> Macros externes
pub use async_recursion::async_recursion;
pub use async_trait::async_trait;

// --> I/O
pub use std::io::{BufRead, Read, Seek, Write};
pub use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

// --> Collections & Types
pub use lru::LruCache;
pub use std::cmp::Ordering;
pub use std::collections::{BTreeMap, HashMap, HashSet};
pub use std::fmt;
pub use std::hash::Hash;
pub use std::num::NonZeroUsize;
pub use std::thread;
pub use std::time::{Duration, Instant};

// --> Regex
pub use regex::Regex;
