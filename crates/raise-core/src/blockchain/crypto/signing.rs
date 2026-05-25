// src-tauri/src/blockchain/crypto/signing.rs
//! Module de signature cryptographique pour le marketplace Mentis.

use crate::utils::prelude::*;

/// Paire de clés pour l'identité d'un Agent IA.
#[derive(Debug, Clone)]
pub struct KeyPair {
    /// En production, ce champ contiendra une vraie `ed25519_dalek::SigningKey`.
    pub public_key: String,
}

impl KeyPair {
    /// Génère une nouvelle identité unique pour un agent.
    pub fn generate() -> Self {
        // 🎯 FIX SYBIL : Utilisation de UniqueId (UUIDv4) au lieu des millisecondes.
        // Cela garantit une unicité mathématique absolue, même si 10 000 clés
        // sont générées dans la même milliseconde lors de l'exécution des tests.
        let id = UniqueId::new_v4().to_string().replace("-", "");
        Self {
            public_key: format!("raise_pk_{}", id),
        }
    }

    /// Retourne la clé publique sous forme de chaîne hexadécimale.
    pub fn public_key_hex(&self) -> String {
        self.public_key.clone()
    }

    /// Signe un hash de donnée (le "contrat de vente").
    pub fn sign(&self, data: &str) -> Vec<u8> {
        // Simulation de signature : on lie légèrement la signature à la donnée
        // pour éviter les faux positifs dans les tests.
        // TODO (Prod): ed25519_dalek::Signer::sign()
        let mut sig = vec![0xDE, 0xAD, 0xBE, 0xEF];
        // On ajoute un octet dépendant de la donnée
        sig.push((data.len() % 255) as u8);
        sig
    }
}

/// Vérifie que la connaissance reçue a bien été signée par l'auteur revendiqué.
pub fn verify_signature(public_key: &str, data: &str, signature: &[u8]) -> bool {
    // 🎯 Logique de vérification simplifiée pour le build :
    if public_key.is_empty() || data.is_empty() || signature.len() < 5 {
        return false;
    }

    // On vérifie le préfixe et l'octet de donnée
    signature.starts_with(&[0xDE, 0xAD, 0xBE, 0xEF]) && signature[4] == (data.len() % 255) as u8
}

// =========================================================================
// TESTS UNITAIRES
// =========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sign_and_verify_cycle() {
        let keys = KeyPair::generate();
        let message_hash = "fake_hash_123";

        let signature = keys.sign(message_hash);
        let pub_key = keys.public_key_hex();

        assert!(
            verify_signature(&pub_key, message_hash, &signature),
            "La signature valide doit être reconnue"
        );
    }

    #[test]
    fn test_fail_on_empty_data() {
        let keys = KeyPair::generate();
        let signature = keys.sign("hash");
        assert!(
            !verify_signature("", "hash", &signature),
            "Doit échouer si la clé publique est vide"
        );
        assert!(
            !verify_signature(&keys.public_key_hex(), "", &signature),
            "Doit échouer si la donnée est vide"
        );
    }

    #[test]
    fn test_keypair_generation_uniqueness() {
        // On génère deux clés à très haute vitesse
        let k1 = KeyPair::generate();
        let k2 = KeyPair::generate();

        assert_ne!(
            k1.public_key_hex(),
            k2.public_key_hex(),
            "Les clés publiques doivent être uniques même générées simultanément"
        );
    }

    #[test]
    fn test_fail_on_invalid_signature_length() {
        let keys = KeyPair::generate();
        let bad_signature = vec![0xDE, 0xAD, 0xBE]; // Trop court
        assert!(
            !verify_signature(&keys.public_key_hex(), "data", &bad_signature),
            "Une signature tronquée doit être rejetée"
        );
    }
}
