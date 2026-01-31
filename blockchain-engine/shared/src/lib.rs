use serde::{Deserialize, Serialize};
use serde_json::Value;

// ==============================================================================
// MODULE GRPC (Généré automatiquement depuis shared/protos/chaincode.proto)
// ==============================================================================
pub mod chaincode {
    // La macro magique qui inclut le code généré par build.rs
    tonic::include_proto!("chaincode");
}

// ==============================================================================
// STRUCTURES PATRIMOINE (JSON-LD & Stockage)
// ==============================================================================

/// Représente une entité certifiée sur la blockchain.
/// Cette structure fait le pont entre le stockage JSON-LD local et le ledger.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct SemanticEvidence {
    /// URI unique de l'entité (ex: "urn:uuid:...")
    pub id: String,

    /// Référence au schéma utilisé pour la validation (ex: l'URL du actor.schema.json)
    /// Cela permet au Chaincode de retrouver la règle dans son propre registre.
    pub schema_id: String,

    /// Le type sémantique Arcadia issu du JSON-LD (@type)
    pub semantic_type: String,

    /// Hash SHA-256 de l'objet complet tel qu'il existe dans le WAL/json-db.
    /// C'est l'empreinte numérique qui rend la donnée infalsifiable.
    pub content_hash: String,

    /// Métadonnées critiques extraites (Framing) pour l'audit direct sur le ledger.
    /// On n'y met que le strict nécessaire (ex: auteur, état de validation).
    pub metadata: Value,

    /// Timestamp de création de la preuve (Unix Epoch)
    pub timestamp: u64,
}

/// Structure pour l'enregistrement d'un schéma dans le registre de la blockchain.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct SchemaAnchor {
    pub uri: String,              // L'identifiant du schéma ($id)
    pub version: String,          // x_schemaVersion
    pub schema_definition: Value, // Le contenu du schéma JSON pour validation gérée par le chaincode
    pub hash: String,             // Hash du fichier de schéma lui-même
}

// ==============================================================================
// TESTS UNITAIRES
// ==============================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // Test 1 : Vérifie que les structures "Patrimoine" se sérialisent correctement en JSON
    #[test]
    fn test_semantic_evidence_serialization() {
        let evidence = SemanticEvidence {
            id: "urn:uuid:1234".to_string(),
            schema_id: "schema:actor".to_string(),
            semantic_type: "Person".to_string(),
            content_hash: "abc_hash_123".to_string(),
            metadata: json!({ "author": "Didier" }),
            timestamp: 1600000000,
        };

        // Sérialisation
        let json_str = serde_json::to_string(&evidence).expect("Should serialize");
        assert!(json_str.contains("urn:uuid:1234"));
        assert!(json_str.contains("abc_hash_123"));

        // Désérialisation
        let evidence_back: SemanticEvidence =
            serde_json::from_str(&json_str).expect("Should deserialize");
        assert_eq!(evidence, evidence_back);
    }

    // Test 2 : Vérifie la structure SchemaAnchor
    #[test]
    fn test_schema_anchor_serialization() {
        let anchor = SchemaAnchor {
            uri: "schema:test".to_string(),
            version: "1.0".to_string(),
            schema_definition: json!({"type": "object"}),
            hash: "hash_schema".to_string(),
        };

        let json_str = serde_json::to_string(&anchor).unwrap();
        let anchor_back: SchemaAnchor = serde_json::from_str(&json_str).unwrap();

        assert_eq!(anchor, anchor_back);
    }

    // Test 3 : Smoke Test pour le code généré par gRPC
    #[test]
    fn test_grpc_types_generation() {
        // CORRECTION ICI :
        // L'enum Type est générée DANS un module portant le nom du message (chaincode_message)
        use crate::chaincode::chaincode_message;
        use crate::chaincode::ChaincodeMessage; // On importe le module interne

        let msg = ChaincodeMessage {
            // On accède à l'enum via son module parent
            r#type: chaincode_message::Type::Transaction as i32,
            timestamp_seconds: 123456,
            payload: vec![1, 2, 3],
            txid: "test_tx_id".to_string(),
        };

        assert_eq!(msg.timestamp_seconds, 123456);
        assert_eq!(msg.txid, "test_tx_id");
    }
}
