// FICHIER : src-tauri/src/workflow_engine/mandate.rs

use crate::json_db::collections::manager::CollectionsManager;
use crate::utils::{AppError, Result};
use anyhow::{anyhow, Context};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;

// --- STRUCTURES DU MANDAT ---

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Mandate {
    // L'ID est injecté par la DB, mais utile pour le contexte
    #[serde(default)]
    pub id: String,

    pub meta: MandateMeta,
    pub governance: Governance,
    pub hard_logic: HardLogic,
    pub observability: Observability,

    // Signature cryptographique (Optionnelle à la création, requise en prod)
    pub signature: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MandateMeta {
    pub author: String,
    pub status: String,
    pub version: String,
}

/// Stratégie de gouvernance fortement typée
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Strategy {
    SafetyFirst,
    Performance,
    Balanced,
    Test, // Ajouté pour compatibilité avec vos tests existants
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Governance {
    pub strategy: Strategy,
    #[serde(default)]
    pub condorcet_weights: HashMap<String, f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HardLogic {
    pub vetos: Vec<VetoRule>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VetoRule {
    pub rule: String,
    pub active: bool,
    pub action: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Observability {
    pub heartbeat_ms: u64,
    #[serde(default)]
    pub metrics: Vec<String>,
}

// --- IMPLÉMENTATION ET LOGIQUE MÉTIER ---

impl Mandate {
    /// Charge un mandat depuis la JSON-DB et le convertit en structure Rust stricte.
    /// C'est la méthode "Pont" qui garantit que les données dynamiques sont conformes au code.
    pub fn fetch_from_store(manager: &CollectionsManager, mandate_id: &str) -> Result<Self> {
        // 1. Récupération du document brut (Value)
        let doc_value = manager
            .get_document("mandates", mandate_id)?
            .ok_or_else(|| anyhow!("Mandat introuvable dans la base (ID: {})", mandate_id))
            .map_err(AppError::from)?;

        // 2. Désérialisation Validante (Le "Crash Test")
        // Si le JSON ne respecte pas les structures Rust ci-dessus, ceci échouera.
        let mandate: Mandate = serde_json::from_value(doc_value)
            .context("Échec critique d'intégrité : Le mandat stocké ne correspond pas à la structure interne du moteur")
            .map_err(AppError::from)?;

        // 3. Validation Logique supplémentaire
        mandate.validate_business_logic().map_err(AppError::from)?;

        Ok(mandate)
    }

    /// Vérifie les règles métier internes au mandat une fois chargé
    fn validate_business_logic(&self) -> anyhow::Result<()> {
        // Règle : En mode SAFETY_FIRST, il faut au moins un veto actif
        if self.governance.strategy == Strategy::SafetyFirst
            && !self.hard_logic.vetos.iter().any(|v| v.active)
        {
            return Err(anyhow!(
                "INCOHÉRENCE : Stratégie 'SAFETY_FIRST' sans veto actif."
            ));
        }
        Ok(())
    }

    /// Vérifie la signature cryptographique du mandat.
    /// (Code existant conservé et adapté pour utiliser AppError)
    pub fn verify_signature(&self, public_key_hex: &str) -> Result<bool> {
        if self.signature.is_none() {
            return Ok(false);
        }
        let sig_hex = self.signature.as_ref().unwrap();

        // 1. Payload Canonique (JSON sans la signature)
        let mut unsigned_clone = self.clone();
        unsigned_clone.signature = None;
        // On exclut aussi l'ID s'il est injecté par la DB et pas signé à l'origine
        // (Dépend de votre stratégie de signature, ici on garde la structure telle quelle)

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

// --- TESTS ---

// FICHIER : src-tauri/src/workflow_engine/mandate.rs (Section Tests Uniquement)

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};
    use serde_json::json;
    // On suppose que json_db expose des utils de test (mock ou in-memory)
    use crate::json_db::test_utils::init_test_env;

    // Helper pour créer un mandat de test (Adapté au nouveau typage)
    fn create_dummy_mandate() -> Mandate {
        Mandate {
            id: "dummy_id".into(),
            meta: MandateMeta {
                author: "Tester".into(),
                status: "DRAFT".into(),
                version: "1.0".into(),
            },
            governance: Governance {
                strategy: Strategy::Test, // Utilisation de l'enum
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
        // 1. Génération de clés
        let seed: [u8; 32] = rand::random();
        let signing_key = SigningKey::from_bytes(&seed);
        let verifying_key = signing_key.verifying_key();

        // 2. Création et Signature
        let mut mandate = create_dummy_mandate();

        // Note: serde_json::to_string utilisera camelCase ici aussi
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
        let seed: [u8; 32] = rand::random();
        let signing_key = SigningKey::from_bytes(&seed);
        let verifying_key = signing_key.verifying_key();

        let mut mandate = create_dummy_mandate();

        let payload = serde_json::to_string(&mandate).unwrap();
        let mut hasher = Sha256::new();
        hasher.update(payload.as_bytes());
        let hash = hasher.finalize();
        let signature = signing_key.sign(&hash);
        mandate.signature = Some(hex::encode(signature.to_bytes()));

        // HACK: Modification malveillante (On change la stratégie)
        mandate.governance.strategy = Strategy::Performance;

        let pub_key_hex = hex::encode(verifying_key.to_bytes());
        let is_valid = mandate
            .verify_signature(&pub_key_hex)
            .expect("Erreur interne verify");

        assert!(!is_valid, "Un mandat modifié ne doit PAS être validé");
    }

    // --- NOUVEAUX TESTS D'INTÉGRATION (Bridge) ---

    #[test]
    fn test_fetch_mandate_integration_nominal() {
        // Setup in-memory DB via json_db::test_utils
        let env = init_test_env();
        let manager = CollectionsManager::new(&env.storage, &env.space, &env.db);

        // CORRECTION : On définit l'objet entier directement, ID inclus.
        // On évite la syntaxe "...valid_json" qui n'existe pas en Rust macro.
        let full_json = json!({
            "id": "man_01",
            "meta": { "author": "Integration", "version": "2.0", "status": "ACTIVE" },
            "governance": {
                "strategy": "SAFETY_FIRST",
                "condorcetWeights": { "security": 10.0 }
            },
            "hardLogic": {
                "vetos": [{ "rule": "MAX_TEMP", "active": true, "action": "STOP" }]
            },
            "observability": { "heartbeatMs": 100 }
        });

        // Insertion brute (simule le stockage)
        manager.insert_raw("mandates", &full_json).unwrap();

        // Test du Pont
        let result = Mandate::fetch_from_store(&manager, "man_01");
        assert!(
            result.is_ok(),
            "Le chargement devrait réussir : {:?}",
            result.err()
        );

        let mandate = result.unwrap();
        assert_eq!(mandate.governance.strategy, Strategy::SafetyFirst);
        assert_eq!(mandate.hard_logic.vetos[0].rule, "MAX_TEMP");
    }

    #[test]
    fn test_fetch_mandate_schema_mismatch() {
        let env = init_test_env();
        let manager = CollectionsManager::new(&env.storage, &env.space, &env.db);

        // JSON invalide (manque 'hardLogic')
        let bad_json = json!({
            "id": "man_broken",
            "meta": { "author": "Hacker", "version": "0.0", "status": "DRAFT" },
            "governance": { "strategy": "PERFORMANCE" }
        });

        manager.insert_raw("mandates", &bad_json).unwrap();

        let result = Mandate::fetch_from_store(&manager, "man_broken");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Échec critique d'intégrité"));
    }
}
