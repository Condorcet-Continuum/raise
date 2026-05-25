// FICHIER : src-tauri/src/utils/context/mod.rs

pub mod i18n;
pub mod logger;
pub mod session;

// =========================================================================
// FAÇADE `context` : État Global et Observabilité (AI-Ready)
// =========================================================================
// 🤖 IA NOTE : Ce module gère le "Contexte d'Exécution" de l'application :
// - Qui utilise l'application ? (Session)
// - Dans quelle langue ? (i18n)
// - Que se passe-t-il ? (Logger)
// L'état est souvent protégé par des verrous asynchrones (AsyncRwLock).

pub use crate::utils::data::config::AppConfig;
pub use i18n::{init_i18n, t};
pub use logger::init_logging;
pub use session::{Session, SessionManager, SessionStatus};
