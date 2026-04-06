// FICHIER : src-tauri/src/workflow_engine/mandate.rs
use crate::json_db::collections::manager::CollectionsManager;
use crate::rules_engine::analyzer::Analyzer;
use crate::rules_engine::ast::Expr;
use crate::utils::prelude::*;
use ed25519_dalek::{Signature, Verifier, VerifyingKey};

// --- NOUVELLES STRUCTURES DE GOUVERNANCE (Alignées sur les schémas) ---

#[derive(Debug, Clone, Serializable, Deserializable)]
pub struct Permission {
    pub handle: String,
    pub name: I18nString,
    pub service: String,
    pub action: ActionType,
    pub conditions: Option<JsonValue>,
}

#[derive(Debug, Clone, Copy, Serializable, Deserializable, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ActionType {
    Read,
    Create,
    Update,
    Delete,
    Execute,
    Sign,
    Approve,
}

#[derive(Debug, Clone, Serializable, Deserializable)]
pub struct Role {
    pub handle: String,
    pub name: I18nString,
    pub granted_permissions: Vec<UniqueId>,
    pub inherited_roles: Vec<UniqueId>,
    pub status: String,
}

#[derive(Debug, Clone, Serializable, Deserializable)]
pub struct Mandator {
    pub handle: String,
    pub nature: String, // "HUMAN"
    pub user_ids: Vec<UniqueId>,
    pub assigned_roles: Vec<UniqueId>,
    pub authority_scope: AuthorityScope,
    pub authorized_layers: Vec<ArcadiaLayer>,
    pub public_key: String,
    pub status: String,
}

#[derive(Debug, Clone, Serializable, Deserializable)]
pub struct AuthorityScope {
    pub organizations: Vec<UniqueId>,
    pub domains: Vec<UniqueId>,
    pub teams: Vec<UniqueId>,
    pub databases: Vec<UniqueId>,
}

#[derive(Debug, Clone, Copy, Serializable, Deserializable, PartialEq)]
pub enum ArcadiaLayer {
    OA,
    SA,
    LA,
    PA,
    EPBS,
    DATA,
    TRANSVERSE,
}

// --- MISE À JOUR DE LA STRUCTURE MANDATE EXISTANTE ---

#[derive(Debug, Clone, Serializable, Deserializable)]
#[serde(rename_all = "camelCase")] // 🎯 REQUIS : Le schéma utilise hardLogic, observability...
pub struct Mandate {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub _id: Option<String>,
    pub handle: String,
    pub name: I18nString, // 🎯 Utilisation propre du type que nous avons mis dans le prelude
    pub meta: MandateMeta,
    pub governance: Governance,
    pub hard_logic: HardLogic,
    pub observability: Observability,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
}

#[derive(Debug, Clone, Serializable, Deserializable)]
// 🎯 AUCUN RENAME ICI : Le schéma exige "mandator_id" en snake_case strict
pub struct MandateMeta {
    pub mandator_id: UniqueId,
    pub status: String,
    pub version: String,
}

#[derive(Debug, Clone, Serializable, Deserializable, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Strategy {
    SafetyFirst,
    Performance,
    Balanced,
    Test,
}

#[derive(Debug, Clone, Serializable, Deserializable)]
#[serde(rename_all = "camelCase")] // 🎯 REQUIS : Le schéma utilise condorcetWeights
pub struct Governance {
    pub strategy: Strategy,
    pub condorcet_weights: UnorderedMap<String, f64>,
}

#[derive(Debug, Clone, Serializable, Deserializable)]
#[serde(rename_all = "camelCase")]
pub struct HardLogic {
    pub vetos: Vec<VetoRule>,
}

#[derive(Debug, Clone, Serializable, Deserializable)]
#[serde(rename_all = "camelCase")]
pub struct VetoRule {
    pub rule: String,
    pub active: bool,
    pub action: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ast: Option<JsonValue>,
}

#[derive(Debug, Clone, Serializable, Deserializable)]
#[serde(rename_all = "camelCase")] // 🎯 REQUIS : Le schéma utilise heartbeatMs
pub struct Observability {
    pub heartbeat_ms: u64,
}

#[derive(Debug)]
pub struct VetoAnalysis {
    pub rule_name: String,
    pub status: RaiseResult<Vec<String>>,
}

// --- LOGIQUE MÉTIER ---

impl Mandate {
    pub async fn fetch_from_store(
        manager: &CollectionsManager<'_>,
        handle: &str,
    ) -> RaiseResult<Self> {
        let mandate_result = manager.get_document("mandates", handle).await;
        let doc = match mandate_result {
            Ok(Some(document)) => document,
            Ok(None) => raise_error!(
                "ERR_WF_MANDATE_NOT_FOUND",
                context = json_value!({
                    "handle": handle,
                    "action": "resolve_mandate",
                    "hint": "L'identifiant est inconnu ou le mandat a été révoqué."
                })
            ),
            Err(e) => raise_error!(
                "ERR_WF_MANDATE_DB_ACCESS",
                context = json_value!({
                    "handle": handle,
                    "db_error": e.to_string(),
                })
            ),
        };

        let mut mandate: Mandate = match json::deserialize_from_value(doc) {
            Ok(m) => m,
            Err(e) => raise_error!(
                "ERR_WF_MANDATE_CORRUPT",
                context = json_value!({
                    "handle": handle,
                    "serialization_error": e.to_string(),
                })
            ),
        };

        mandate._id = Some(handle.to_string());
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
        json::serialize_to_string(&clone).unwrap_or_default()
    }

    pub fn analyze_vetos(&self) -> Vec<VetoAnalysis> {
        let mut results = Vec::new();

        for veto in &self.hard_logic.vetos {
            let rule_name = veto.rule.clone();

            let status = (|| -> RaiseResult<Vec<String>> {
                let ast_val = veto.ast.as_ref().ok_or_else(|| {
                    build_error!(
                        "ERR_AST_MISSING",
                        error = "Aucun AST défini pour cette règle",
                        context = json_value!({ "rule": rule_name })
                    )
                })?;

                let expr: Expr = json::deserialize_from_value(ast_val.clone()).map_err(|e| {
                    build_error!(
                        "ERR_JSON_DESERIALIZE",
                        error = format!("Échec du parsing AST : {}", e),
                        context = json_value!({ "rule": rule_name })
                    )
                })?;

                Analyzer::validate_depth(&expr, 50)?;
                let deps = Analyzer::get_dependencies(&expr).into_iter().collect();
                Ok(deps)
            })();

            results.push(VetoAnalysis { rule_name, status });
        }

        results
    }
}

// ============================================================================
// TESTS UNITAIRES
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json_db::test_utils::init_test_env;
    use crate::utils::testing::DbSandbox;

    #[async_test]
    async fn test_fetch_mandate_success() {
        let env = init_test_env().await;
        let manager = CollectionsManager::new(&env.sandbox.storage, &env.space, &env.db);
        DbSandbox::mock_db(&manager).await.unwrap();

        let full_json = json_value!({
            "handle": "mandate-core-v1",
            "name": "Mandat Central",
            // 🎯 FIX : Utilisation stricte de mandator_id avec un UUID valide
            "meta": { "mandator_id": "00000000-0000-0000-0000-000000000000", "version": "1.0", "status": "ACTIVE" },
            "governance": {
                "strategy": "SAFETY_FIRST",
                "condorcetWeights": { "agent_security": 10.0 }
            },
            "hardLogic": {
                "vetos": [{ "rule": "MAX_TEMP", "active": true, "action": "STOP" }]
            },
            "observability": { "heartbeatMs": 100 }
        });

        manager
            .create_collection(
                "mandates",
                "db://_system/_system/schemas/v1/db/generic.schema.json",
            )
            .await
            .unwrap();
        manager
            .upsert_document("mandates", full_json)
            .await
            .unwrap();

        let result = Mandate::fetch_from_store(&manager, "mandate-core-v1").await;
        assert!(result.is_ok());

        let mandate = result.unwrap();
        assert_eq!(mandate.handle, "mandate-core-v1");
        assert_eq!(mandate.governance.strategy, Strategy::SafetyFirst);
        assert!(mandate.hard_logic.vetos[0].ast.is_none());
    }

    #[async_test]
    async fn test_fetch_mandate_with_ast() {
        let env = init_test_env().await;
        let manager = CollectionsManager::new(&env.sandbox.storage, &env.space, &env.db);
        DbSandbox::mock_db(&manager).await.unwrap();

        let ast_json = json_value!({
            "gt": [{"var": "temp"}, {"val": 100.0}]
        });

        let full_json = json_value!({
            "handle": "mandate-perf-v2",
            "name": "Mandat Performance",
            // 🎯 FIX : Utilisation stricte de mandator_id avec un UUID valide
            "meta": { "mandator_id": "00000000-0000-0000-0000-000000000000", "version": "2.0", "status": "ACTIVE" },
            "governance": {
                "strategy": "PERFORMANCE",
                "condorcetWeights": {}
            },
            "hardLogic": {
                "vetos": [{
                    "rule": "DYNAMIC_TEMP",
                    "active": true,
                    "action": "STOP",
                    "ast": ast_json
                }]
            },
            "observability": { "heartbeatMs": 100 }
        });

        manager
            .create_collection(
                "mandates",
                "db://_system/_system/schemas/v1/db/generic.schema.json",
            )
            .await
            .unwrap();
        manager
            .upsert_document("mandates", full_json)
            .await
            .unwrap();

        let result = Mandate::fetch_from_store(&manager, "mandate-perf-v2").await;
        assert!(result.is_ok());
        let mandate = result.unwrap();

        assert!(mandate.hard_logic.vetos[0].ast.is_some());
    }

    #[async_test]
    async fn test_fetch_mandate_schema_mismatch() {
        let env = init_test_env().await;
        let manager = CollectionsManager::new(&env.sandbox.storage, &env.space, &env.db);
        DbSandbox::mock_db(&manager).await.unwrap();

        let bad_json = json_value!({
            "handle": "broken",
            "meta": { "mandator_id": "00000000-0000-0000-0000-000000000000", "version": "0.0", "status": "DRAFT" },
            "governance": { "strategy": "PERFORMANCE" }
        });

        manager
            .create_collection(
                "mandates",
                "db://_system/_system/schemas/v1/db/generic.schema.json",
            )
            .await
            .unwrap();
        manager.upsert_document("mandates", bad_json).await.unwrap();

        let result = Mandate::fetch_from_store(&manager, "broken").await;
        assert!(result.is_err());
    }

    #[test]
    fn test_analyze_vetos_full_success() {
        let ast = json_value!({ "gt": [{"var": "pa.brakes.temp"}, {"val": 120.0}] });

        let mandate = Mandate {
            _id: None,
            handle: "test-mandate".into(),
            name: "Test Mandate".into(),
            meta: MandateMeta {
                mandator_id: UniqueId::nil(),
                status: "ACTIVE".into(),
                version: "1.0".into(),
            },
            governance: Governance {
                strategy: Strategy::SafetyFirst,
                condorcet_weights: UnorderedMap::new(),
            },
            hard_logic: HardLogic {
                vetos: vec![VetoRule {
                    rule: "TEMP_MAX".into(),
                    active: true,
                    action: "STOP".into(),
                    ast: Some(ast),
                }],
            },
            observability: Observability { heartbeat_ms: 1000 },
            signature: None,
        };

        let results = mandate.analyze_vetos();
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_analyze_vetos_invalid_ast_structure() {
        let mandate = Mandate {
            hard_logic: HardLogic {
                vetos: vec![VetoRule {
                    rule: "BROKEN_RULE".into(),
                    active: true,
                    action: "STOP".into(),
                    ast: Some(json_value!({ "not_an_operator": 123 })),
                }]
            },
            // 🎯 FIX : Un JSON brut 100% conforme pour tester la désérialisation
            ..serde_json::from_str(r#"{"handle":"test","name":"Test","meta":{"mandator_id":"00000000-0000-0000-0000-000000000000","status":"ACTIVE","version":"1.0"},"governance":{"strategy":"SAFETY_FIRST","condorcetWeights":{}},"hardLogic":{"vetos":[]},"observability":{"heartbeatMs":100}}"#).unwrap()
        };

        let results = mandate.analyze_vetos();
        assert!(results[0].status.is_err());
    }
}
