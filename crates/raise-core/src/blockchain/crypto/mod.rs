// src-tauri/src/blockchain/crypto/mod.rs
//! Module cryptographique du réseau souverain Mentis.
//! Gère le hachage déterministe, les arbres de Merkle et les signatures asymétriques.

pub mod hashing;
pub mod signing;

// =========================================================================
// FAÇADE CRYPTOGRAPHIQUE RAISE
// =========================================================================
// On expose les primitives essentielles au niveau du module `crypto`
// pour garantir un couplage faible et simplifier les imports dans `consensus` et `storage`.

pub use hashing::{calculate_hash, calculate_merkle_root};
pub use signing::{verify_signature, KeyPair};

// =========================================================================
// TESTS UNITAIRES (Audit de Visibilité)
// =========================================================================

#[cfg(test)]
mod tests {
    // On importe uniquement le module racine crypto pour vérifier la façade
    use super::*;

    #[test]
    fn test_crypto_facade_visibility() {
        // Si ce test compile, cela prouve que les modules enfants sont correctement
        // encapsulés et que la façade expose les bonnes primitives.

        // Test de la façade Hashing
        let json_data = crate::utils::prelude::json_value!({"test": "facade"});
        let hash = calculate_hash(&json_data);
        let root = calculate_merkle_root(&[hash]);
        assert_eq!(root.len(), 64);

        // Test de la façade Signing
        let keys = KeyPair::generate();
        let pub_key = keys.public_key_hex();
        let sig = keys.sign("facade_test");
        assert!(verify_signature(&pub_key, "facade_test", &sig));
    }
}
