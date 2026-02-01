use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};

pub struct KeyPair {
    signing_key: SigningKey,
}

impl KeyPair {
    /// Génère une nouvelle paire de clés pour un ingénieur (foaf:Agent)
    /// Utilise le générateur interne compatible avec la version de rand détectée.
    pub fn generate() -> Self {
        let mut bytes = [0u8; 32];

        // Correction du warning : rand 0.9 utilise rand::rng() au lieu de thread_rng()
        let mut rng = rand::rng();
        use rand::RngCore;
        rng.fill_bytes(&mut bytes);

        let signing_key = SigningKey::from_bytes(&bytes);
        Self { signing_key }
    }

    /// Signe un hash de message (ex: le hash d'un ArcadiaCommit)
    pub fn sign(&self, message_hash: &str) -> Vec<u8> {
        let signature = self.signing_key.sign(message_hash.as_bytes());
        signature.to_bytes().to_vec()
    }

    /// Retourne la clé publique sous forme d'hexadécimal (ID de l'agent)
    pub fn public_key_hex(&self) -> String {
        hex::encode(self.signing_key.verifying_key().to_bytes())
    }
}

/// Vérifie si une signature est valide pour un message donné
pub fn verify_signature(public_key_hex: &str, message_hash: &str, signature_bytes: &[u8]) -> bool {
    let Ok(public_key_bytes) = hex::decode(public_key_hex) else {
        return false;
    };
    let Ok(verifying_key) = VerifyingKey::try_from(&public_key_bytes[..]) else {
        return false;
    };

    // Tentative de conversion de la signature (Ed25519 = 64 octets)
    let Ok(sig_array) = <[u8; 64]>::try_from(signature_bytes) else {
        return false;
    };
    let signature = Signature::from_bytes(&sig_array);

    verifying_key
        .verify(message_hash.as_bytes(), &signature)
        .is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sign_and_verify() {
        let keys = KeyPair::generate();
        let message_hash = "9ec3e8fe26122da4994f08f0dc21e00f042dad34b74aa9f794d18e6d8ce85b1c";

        let signature = keys.sign(message_hash);
        let pub_key = keys.public_key_hex();

        assert!(verify_signature(&pub_key, message_hash, &signature));
    }

    #[test]
    fn test_fail_verification_on_wrong_message() {
        let keys = KeyPair::generate();
        let signature = keys.sign("hash_a");
        let pub_key = keys.public_key_hex();

        assert!(!verify_signature(&pub_key, "hash_b", &signature));
    }
}
