// FICHIER : src-tauri/src/utils/io/io_traits.rs

/// 🤖 IA NOTE : Trait pour l'écriture synchrone (bloquante).
/// À utiliser pour les buffers (`Vec<u8>`) ou les flux standards (`stdout`).
pub use std::io::Write as SyncWrite;

/// 🤖 IA NOTE : Trait pour la lecture synchrone (bloquante).
pub use std::io::Read as SyncRead;

/// 🤖 IA NOTE : Trait pour le déplacement dans un flux synchrone (curseur).
pub use std::io::Seek as SyncSeek;

/// 🤖 IA NOTE : Trait pour la lecture ligne par ligne avec mise en mémoire tampon.
pub use std::io::BufRead as SyncBufRead;
