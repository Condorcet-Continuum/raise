// FICHIER : src-tauri/src/workflow_engine/mandate.rs
use crate::json_db::collections::manager::CollectionsManager;
use crate::rules_engine::analyzer::Analyzer;
use crate::rules_engine::ast::Expr;
use crate::utils::prelude::*;
use ed25519_dalek::{Signature, Verifier, VerifyingKey};

// --- STRUCTURES DU MANDAT (Alignées sur mandate.schema.json) ---

#[derive(Debug, Clone, Serializable, Deserializable)]
#[serde(rename_all = "camelCase")]
pub struct Mandate {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub _id: Option<String>,
    pub handle: String,
    pub name: JsonValue, // Supporte string ou i18n object
    pub meta: MandateMeta,
    pub governance: Governance,
    pub hard_logic: HardLogic,
    pub observability: Observability,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
}

#[derive(Debug, Clone, Serializable, Deserializable)]
#[serde(rename_all = "camelCase")]
pub struct MandateMeta {
    pub author: String,
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
#[serde(rename_all = "camelCase")]
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
    pub ast: Option<JsonValue>, // L'arbre syntaxique pour le Rules Engine
}

#[derive(Debug, Clone, Serializable, Deserializable)]
#[serde(rename_all = "camelCase")]
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
                    // 🎯 FIX 2 : Utilisation stricte de la macro build_error! du socle Raise
                    build_error!(
                        "ERR_AST_MISSING",
                        error = "Aucun AST défini pour cette règle",
                        context = json_value!({ "rule": rule_name })
                    )
                })?;

                let expr: Expr = json::deserialize_from_value(ast_val.clone()).map_err(|e| {
                    // 🎯 FIX 2 : Utilisation de build_error! pour mapper l'erreur JSON
                    build_error!(
                        "ERR_JSON_DESERIALIZE",
                        error = format!("Échec du parsing AST : {}", e),
                        context = json_value!({ "rule": rule_name })
                    )
                })?;

                // Validation de la profondeur (Sécurité Fail-Safe)
                Analyzer::validate_depth(&expr, 50)?;

                // Extraction des dépendances
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

        // JSON strict correspondant à mandate.schema.json
        let full_json = json_value!({
            "handle": "mandate-core-v1",
            "name": "Mandat Central",
            "meta": { "author": "System Admin", "version": "1.0", "status": "ACTIVE" },
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
        // Vérification rétrocompatibilité : ast doit être None si non fourni
        assert!(mandate.hard_logic.vetos[0].ast.is_none());
    }

    #[async_test]
    async fn test_fetch_mandate_with_ast() {
        let env = init_test_env().await;
        let manager = CollectionsManager::new(&env.sandbox.storage, &env.space, &env.db);
        DbSandbox::mock_db(&manager).await.unwrap();

        // Une règle dynamique (AST) injectée pour le Rules Engine
        let ast_json = json_value!({
            "gt": [{"var": "temp"}, {"val": 100.0}]
        });

        let full_json = json_value!({
            "handle": "mandate-perf-v2",
            "name": "Mandat Performance",
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
                    "ast": ast_json // L'AST est bien injecté
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

        // L'AST doit être correctement désérialisé
        assert!(mandate.hard_logic.vetos[0].ast.is_some());
        let parsed_ast = mandate.hard_logic.vetos[0].ast.as_ref().unwrap();
        assert!(parsed_ast.get("gt").is_some());
    }

    #[async_test]
    async fn test_fetch_mandate_schema_mismatch() {
        let env = init_test_env().await;
        let manager = CollectionsManager::new(&env.sandbox.storage, &env.space, &env.db);
        DbSandbox::mock_db(&manager).await.unwrap();

        // Un JSON corrompu ou incomplet par rapport au schéma
        let bad_json = json_value!({
            "handle": "broken",
            "meta": { "author": "Hacker", "version": "0.0", "status": "DRAFT" },
            "governance": { "strategy": "PERFORMANCE" } // Il manque hardLogic, observability, etc.
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

        if let Err(e) = result {
            assert!(
                e.to_string().contains("ERR_WF_MANDATE_CORRUPT"),
                "Doit renvoyer une erreur de corruption de payload"
            );
        }
    }

    #[test]
    fn test_analyze_vetos_full_success() {
        let ast = json_value!({ "gt": [{"var": "pa.brakes.temp"}, {"val": 120.0}] });

        let mandate = Mandate {
            _id: None,
            handle: "test-mandate".into(),
            name: json_value!("Test"),
            meta: MandateMeta {
                author: "sys".into(),
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
        assert_eq!(results[0].rule_name, "TEMP_MAX");

        let deps = results[0].status.as_ref().unwrap();
        assert!(deps.contains(&"pa.brakes.temp".to_string()));
    }

    #[test]
    fn test_analyze_vetos_invalid_ast_structure() {
        let mandate = Mandate {
            hard_logic: HardLogic {
                vetos: vec![VetoRule {
                    rule: "BROKEN_RULE".into(),
                    active: true,
                    action: "STOP".into(),
                    ast: Some(json_value!({ "not_an_operator": 123 })), // AST Invalide
                }]
            },
            // Hack pour instancier rapidement le reste (en test)
            ..serde_json::from_str("{\"handle\":\"test\",\"name\":\"\",\"meta\":{\"author\":\"\",\"status\":\"\",\"version\":\"\"},\"governance\":{\"strategy\":\"SAFETY_FIRST\",\"condorcetWeights\":{}},\"hardLogic\":{\"vetos\":[]},\"observability\":{\"heartbeatMs\":100}}").unwrap()
        };

        let results = mandate.analyze_vetos();
        assert!(results[0].status.is_err());

        // 🎯 FIX 3 : On vérifie l'erreur via l'affichage (Display de AppError)
        // ce qui évite de devoir importer la structure interne des erreurs
        let err_msg = results[0].status.as_ref().unwrap_err().to_string();
        assert!(err_msg.contains("ERR_JSON_DESERIALIZE"));
    }
}
