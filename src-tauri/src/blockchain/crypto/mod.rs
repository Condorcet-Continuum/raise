// src-tauri/src/blockchain/crypto/mod.rs

/// Module de hachage déterministe pour les éléments Arcadia (JSON-LD).
pub mod hashing;

/// Module de signature cryptographique (Ed25519) pour l'identité des agents.
pub mod signing;

// On expose les fonctions principales au niveau du module crypto
// pour faciliter leur utilisation ailleurs dans le moteur Raise.
pub use hashing::calculate_hash;
