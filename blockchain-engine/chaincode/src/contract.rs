// blockchain-engine/chaincode/src/contract.rs
// ==================================================================================
// ARCHITECTURE: RAISE CaaS
// ----------------------------------------------------------------------------------
// Logique métier du Smart Contract.
// Utilise les types partagés (raise_shared) pour garantir la cohérence des données.
// ==================================================================================

use crate::chaincode::chaincode_message::Type;
use crate::chaincode::ChaincodeMessage;
use crate::ledger::LedgerContext;
use raise_shared::{SchemaAnchor, SemanticEvidence};
use serde::{Deserialize, Serialize};

// Structure de commande standard (JSON RPC-like)
#[derive(Serialize, Deserialize, Debug)]
pub struct ContractInput {
    pub function: String,
    pub args: serde_json::Value,
}

pub struct RaiseContract;

impl RaiseContract {
    /// POINT D'ENTRÉE PRINCIPAL
    /// Reçoit le contexte du ledger et le message réseau, exécute la logique, et répond.
    pub async fn execute(ctx: &LedgerContext, msg: &ChaincodeMessage) -> ChaincodeMessage {
        // 1. Décodage du payload (JSON -> ContractInput)
        let input: ContractInput = match serde_json::from_slice(&msg.payload) {
            Ok(i) => i,
            Err(e) => return Self::error_response(msg.txid.clone(), &format!("JSON error: {}", e)),
        };

        println!("Invoking contract function: {}", input.function);

        // 2. Routage dynamique
        let result = match input.function.as_str() {
            "register_schema" => Self::router_register_schema(ctx, input.args).await,
            "certify_entity" => Self::router_certify_entity(ctx, input.args).await,
            _ => Err(format!("Unknown function: {}", input.function)),
        };

        // 3. Construction de la réponse gRPC
        match result {
            Ok(_) => Self::success_response(msg.txid.clone(), "Success".as_bytes().to_vec()),
            Err(err_msg) => Self::error_response(msg.txid.clone(), &err_msg),
        }
    }

    // --- ROUTEURS INTERMÉDIAIRES (Adaptateurs JSON -> Structs Typées) ---

    async fn router_register_schema(
        ctx: &LedgerContext,
        args: serde_json::Value,
    ) -> Result<(), String> {
        let anchor: SchemaAnchor = serde_json::from_value(args)
            .map_err(|e| format!("Invalid arguments for register_schema: {}", e))?;
        Self::register_schema(ctx, anchor).await
    }

    async fn router_certify_entity(
        ctx: &LedgerContext,
        args: serde_json::Value,
    ) -> Result<(), String> {
        let evidence: SemanticEvidence = serde_json::from_value(args)
            .map_err(|e| format!("Invalid arguments for certify_entity: {}", e))?;
        Self::certify_entity(ctx, evidence).await
    }

    // --- LOGIQUE MÉTIER (CORE BUSINESS LOGIC) ---

    /// Enregistre un schéma JSON-LD de manière immuable.
    pub async fn register_schema(ctx: &LedgerContext, anchor: SchemaAnchor) -> Result<(), String> {
        // Règle 1 : Unicité de l'URI
        if ctx.exists(&anchor.uri).await? {
            return Err(format!("Schema {} already exists", anchor.uri));
        }

        // (Ici on pourrait ajouter une validation du hash ou de la signature)

        // Écriture
        ctx.put_state(&anchor.uri, &anchor).await
    }

    /// Certifie une entité en vérifiant l'existence de son schéma.
    pub async fn certify_entity(
        ctx: &LedgerContext,
        evidence: SemanticEvidence,
    ) -> Result<(), String> {
        // Règle 1 : Le schéma référencé DOIT exister sur le ledger
        let _schema: SchemaAnchor = ctx
            .get_state(&evidence.schema_id)
            .await?
            .ok_or_else(|| format!("Schema {} not found on ledger", evidence.schema_id))?;

        // Règle 2 : L'entité ne doit pas déjà être certifiée (Immutabilité de l'ID)
        if ctx.exists(&evidence.id).await? {
            return Err(format!("Entity {} is already certified", evidence.id));
        }

        // Écriture
        ctx.put_state(&evidence.id, &evidence).await
    }

    // --- UTILITAIRES DE RÉPONSE ---

    fn success_response(txid: String, payload: Vec<u8>) -> ChaincodeMessage {
        ChaincodeMessage {
            r#type: Type::Completed as i32,
            timestamp_seconds: 0,
            payload,
            txid,
        }
    }

    fn error_response(txid: String, message: &str) -> ChaincodeMessage {
        ChaincodeMessage {
            r#type: Type::Error as i32,
            timestamp_seconds: 0,
            payload: message.as_bytes().to_vec(),
            txid,
        }
    }
}

// ==================================================================================
// TESTS UNITAIRES (Avec MockLedger)
// ==================================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use crate::ledger::{LedgerContext, MockLedger};
    use serde_json::json;
    use std::sync::Arc;

    // Helper pour créer un contexte de test
    fn create_test_context() -> LedgerContext {
        let mock = Arc::new(MockLedger::new());
        LedgerContext::new(mock)
    }

    #[tokio::test]
    async fn test_register_schema_success() {
        let ctx = create_test_context();
        let anchor = SchemaAnchor {
            uri: "schema:actor".to_string(),
            version: "1.0".to_string(),
            schema_definition: json!({"type": "object"}),
            hash: "hash_123".to_string(),
        };

        let result = RaiseContract::register_schema(&ctx, anchor).await;
        assert!(result.is_ok());

        // Vérification de l'écriture
        let saved: Option<SchemaAnchor> = ctx.get_state("schema:actor").await.unwrap();
        assert!(saved.is_some());
        assert_eq!(saved.unwrap().version, "1.0");
    }

    #[tokio::test]
    async fn test_certify_entity_fails_without_schema() {
        let ctx = create_test_context();
        let evidence = SemanticEvidence {
            id: "urn:uuid:entity_1".to_string(),
            schema_id: "schema:missing".to_string(), // Schéma inexistant
            semantic_type: "Actor".to_string(),
            content_hash: "abc".to_string(),
            metadata: json!({}),
            timestamp: 123456,
        };

        let result = RaiseContract::certify_entity(&ctx, evidence).await;
        assert!(result.is_err());
        assert_eq!(
            result.err().unwrap(),
            "Schema schema:missing not found on ledger"
        );
    }

    #[tokio::test]
    async fn test_full_workflow_via_execute() {
        let ctx = create_test_context();

        // 1. Enregistrer le schéma via execute()
        let schema_args = json!({
            "uri": "schema:actor",
            "version": "1.0",
            "schema_definition": {},
            "hash": "h1"
        });
        let msg_schema = ChaincodeMessage {
            r#type: Type::Transaction as i32,
            payload: serde_json::to_vec(&json!({
                "function": "register_schema",
                "args": schema_args
            }))
            .unwrap(),
            txid: "tx_1".to_string(),
            timestamp_seconds: 0,
        };
        let res1 = RaiseContract::execute(&ctx, &msg_schema).await;
        assert_eq!(res1.r#type, Type::Completed as i32);

        // 2. Certifier une entité via execute()
        let entity_args = json!({
            "id": "urn:uuid:e1",
            "schema_id": "schema:actor", // Le schéma existe maintenant
            "semantic_type": "Actor",
            "content_hash": "h2",
            "metadata": {},
            "timestamp": 100
        });
        let msg_entity = ChaincodeMessage {
            r#type: Type::Transaction as i32,
            payload: serde_json::to_vec(&json!({
                "function": "certify_entity",
                "args": entity_args
            }))
            .unwrap(),
            txid: "tx_2".to_string(),
            timestamp_seconds: 0,
        };
        let res2 = RaiseContract::execute(&ctx, &msg_entity).await;
        assert_eq!(res2.r#type, Type::Completed as i32);
    }
}
