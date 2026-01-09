// FICHIER : src-tauri/src/workflow_engine/mandate.rs

use crate::utils::{AppError, Result};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;

// --- STRUCTURES DU MANDAT ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mandate {
    pub meta: MandateMeta,
    pub governance: Governance,
    pub hard_logic: HardLogic,
    pub observability: Observability,
    pub signature: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MandateMeta {
    pub author: String,
    pub status: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Governance {
    pub strategy: String,
    pub condorcet_weights: HashMap<String, f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardLogic {
    pub vetos: Vec<VetoRule>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VetoRule {
    pub rule: String,
    pub active: bool,
    pub action: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Observability {
    pub heartbeat_ms: u64,
    pub metrics: Vec<String>,
}

impl Mandate {
    /// Vérifie la signature cryptographique du mandat.
    pub fn verify_signature(&self, public_key_hex: &str) -> Result<bool> {
        if self.signature.is_none() {
            return Ok(false);
        }
        let sig_hex = self.signature.as_ref().unwrap();

        // 1. Payload Canonique (JSON sans la signature)
        let mut unsigned_clone = self.clone();
        unsigned_clone.signature = None;

        let payload =
            serde_json::to_string(&unsigned_clone).map_err(|e| AppError::from(e.to_string()))?;

        // 2. Hash SHA-256
        let mut hasher = Sha256::new();
        hasher.update(payload.as_bytes());
        let hash = hasher.finalize();

        // 3. Décodage Hexadécimal
        let public_key_bytes = hex::decode(public_key_hex)
            .map_err(|e| AppError::from(format!("Clé publique invalide (Hex): {}", e)))?;

        let signature_bytes = hex::decode(sig_hex)
            .map_err(|e| AppError::from(format!("Signature invalide (Hex): {}", e)))?;

        // 4. Vérification Ed25519
        let verifying_key = VerifyingKey::from_bytes(
            &public_key_bytes
                .try_into()
                .map_err(|_| AppError::from("Taille de clé publique incorrecte".to_string()))?,
        )
        .map_err(|e| AppError::from(format!("Erreur format clé: {}", e)))?;

        let signature = Signature::from_bytes(
            &signature_bytes
                .try_into()
                .map_err(|_| AppError::from("Taille de signature incorrecte".to_string()))?,
        );

        match verifying_key.verify(&hash, &signature) {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};

    // Helper pour créer un mandat de test
    fn create_dummy_mandate() -> Mandate {
        Mandate {
            meta: MandateMeta {
                author: "Tester".into(),
                status: "DRAFT".into(),
                version: "1.0".into(),
            },
            governance: Governance {
                strategy: "TEST".into(),
                condorcet_weights: HashMap::from([("agent_sec".to_string(), 3.0)]),
            },
            hard_logic: HardLogic {
                vetos: vec![VetoRule {
                    rule: "NO_BOOM".into(),
                    active: true,
                    action: "STOP".into(),
                }],
            },
            observability: Observability {
                heartbeat_ms: 50,
                metrics: vec![],
            },
            signature: None,
        }
    }

    #[test]
    fn test_signature_workflow() {
        // 1. Génération de clés via octets aléatoires (Contournement de l'erreur OsRng)
        let seed: [u8; 32] = rand::random();
        let signing_key = SigningKey::from_bytes(&seed);
        let verifying_key = signing_key.verifying_key();

        // 2. Création et Signature
        let mut mandate = create_dummy_mandate();

        let payload = serde_json::to_string(&mandate).unwrap();
        let mut hasher = Sha256::new();
        hasher.update(payload.as_bytes());
        let hash = hasher.finalize();

        let signature = signing_key.sign(&hash);
        mandate.signature = Some(hex::encode(signature.to_bytes()));

        // 3. Vérification
        let pub_key_hex = hex::encode(verifying_key.to_bytes());
        let is_valid = mandate
            .verify_signature(&pub_key_hex)
            .expect("Erreur interne verify");

        assert!(is_valid, "La signature devrait être valide");
    }

    #[test]
    fn test_tampered_mandate_fails() {
        // Setup clés
        let seed: [u8; 32] = rand::random();
        let signing_key = SigningKey::from_bytes(&seed);
        let verifying_key = signing_key.verifying_key();

        let mut mandate = create_dummy_mandate();

        // Signature
        let payload = serde_json::to_string(&mandate).unwrap();
        let mut hasher = Sha256::new();
        hasher.update(payload.as_bytes());
        let hash = hasher.finalize();
        let signature = signing_key.sign(&hash);
        mandate.signature = Some(hex::encode(signature.to_bytes()));

        // HACK: Modification malveillante
        mandate.governance.strategy = "CHAOS_MODE".into();

        // Vérification
        let pub_key_hex = hex::encode(verifying_key.to_bytes());
        let is_valid = mandate
            .verify_signature(&pub_key_hex)
            .expect("Erreur interne verify");

        assert!(!is_valid, "Un mandat modifié ne doit PAS être validé");
    }
}
