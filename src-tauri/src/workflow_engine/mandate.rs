// FICHIER : src-tauri/src/workflow_engine/mandate.rs

use crate::json_db::collections::manager::CollectionsManager;
use crate::utils::{AppError, Result};

use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use serde_json::Value; // AJOUT : Nécessaire pour stocker l'AST JSON
use std::collections::HashMap;

// --- STRUCTURES DU MANDAT ---

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Mandate {
    #[serde(default)]
    pub id: String,
    pub meta: MandateMeta,
    pub governance: Governance,
    pub hard_logic: HardLogic,
    pub observability: Observability,
    pub signature: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MandateMeta {
    pub author: String,
    pub status: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Strategy {
    SafetyFirst,
    Performance,
    Balanced,
    Test,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Governance {
    pub strategy: Strategy,
    pub condorcet_weights: HashMap<String, f64>,
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
    // AJOUT : Stockage optionnel de la règle dynamique (AST JSON)
    // Le "Option" garantit la rétrocompatibilité (pas obligatoire dans le JSON)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ast: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Observability {
    pub heartbeat_ms: u64,
}

// --- LOGIQUE MÉTIER ---

impl Mandate {
    /// Charge un mandat depuis la base de données (Collection "mandates") - ASYNC
    pub async fn fetch_from_store(manager: &CollectionsManager<'_>, id: &str) -> Result<Self> {
        let doc = manager
            .get_document("mandates", id)
            .await
            .map_err(|e| AppError::Database(e.to_string()))?
            .ok_or_else(|| AppError::NotFound(format!("Mandat {} introuvable", id)))?;

        let mut mandate: Mandate = serde_json::from_value(doc).map_err(AppError::Serialization)?;

        mandate.id = id.to_string();
        Ok(mandate)
    }

    pub fn verify_signature(&self, public_key_hex: &str) -> bool {
        let sig_str = match &self.signature {
            Some(s) => s,
            None => return false,
        };

        let public_key_bytes = match hex::decode(public_key_hex) {
            Ok(b) => b,
            Err(_) => return false,
        };

        let verifier =
            match VerifyingKey::from_bytes(&public_key_bytes.try_into().unwrap_or([0u8; 32])) {
                Ok(v) => v,
                Err(_) => return false,
            };

        let signature_bytes = match hex::decode(sig_str) {
            Ok(b) => b,
            Err(_) => return false,
        };

        let signature = match Signature::from_slice(&signature_bytes) {
            Ok(s) => s,
            Err(_) => return false,
        };

        let content = self.canonical_content();
        verifier.verify(content.as_bytes(), &signature).is_ok()
    }

    fn canonical_content(&self) -> String {
        let mut clone = self.clone();
        clone.signature = None;
        serde_json::to_string(&clone).unwrap_or_default()
    }
}

// ============================================================================
// TESTS UNITAIRES
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json_db::test_utils::init_test_env;
    use serde_json::json;

    #[tokio::test]
    async fn test_fetch_mandate_success() {
        let env = init_test_env().await;
        let manager = CollectionsManager::new(&env.storage, &env.space, &env.db);
        manager.init_db().await.unwrap();

        let full_json = json!({
            "id": "man_01",
            "meta": { "author": "System", "version": "1.0", "status": "ACTIVE" },
            "governance": {
                "strategy": "SAFETY_FIRST",
                "condorcetWeights": { "security": 10.0 }
            },
            "hardLogic": {
                "vetos": [{ "rule": "MAX_TEMP", "active": true, "action": "STOP" }]
            },
            "observability": { "heartbeatMs": 100 }
        });

        manager.insert_raw("mandates", &full_json).await.unwrap();

        let result = Mandate::fetch_from_store(&manager, "man_01").await;
        assert!(result.is_ok());

        let mandate = result.unwrap();
        assert_eq!(mandate.governance.strategy, Strategy::SafetyFirst);
        // Vérification rétrocompatibilité : ast doit être None
        assert!(mandate.hard_logic.vetos[0].ast.is_none());
    }

    #[tokio::test]
    async fn test_fetch_mandate_with_ast() {
        let env = init_test_env().await;
        let manager = CollectionsManager::new(&env.storage, &env.space, &env.db);
        manager.init_db().await.unwrap();

        // Une règle dynamique injectée
        let ast_json = json!({
            "Gt": [{"Var": "temp"}, {"Val": 100.0}]
        });

        let full_json = json!({
            "id": "man_ast",
            "meta": { "author": "System", "version": "2.0", "status": "ACTIVE" },
            "governance": {
                "strategy": "PERFORMANCE",
                "condorcetWeights": {}
            },
            "hardLogic": {
                "vetos": [{
                    "rule": "DYNAMIC_TEMP",
                    "active": true,
                    "action": "STOP",
                    "ast": ast_json // Nouveau champ
                }]
            },
            "observability": { "heartbeatMs": 100 }
        });

        manager.insert_raw("mandates", &full_json).await.unwrap();

        let result = Mandate::fetch_from_store(&manager, "man_ast").await;
        assert!(result.is_ok());
        let mandate = result.unwrap();
        // Vérification que l'AST est bien présent
        assert!(mandate.hard_logic.vetos[0].ast.is_some());
    }

    #[tokio::test]
    async fn test_fetch_mandate_schema_mismatch() {
        let env = init_test_env().await;
        let manager = CollectionsManager::new(&env.storage, &env.space, &env.db);
        manager.init_db().await.unwrap();

        let bad_json = json!({
            "id": "man_broken",
            "meta": { "author": "Hacker", "version": "0.0", "status": "DRAFT" },
            "governance": { "strategy": "PERFORMANCE" }
        });

        manager.insert_raw("mandates", &bad_json).await.unwrap();

        let result = Mandate::fetch_from_store(&manager, "man_broken").await;
        assert!(result.is_err());
    }
}
