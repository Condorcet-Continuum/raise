// src-tauri/src/blockchain/dev_client.rs

// 1. Imports corrigés (plus de ::raise::)
use raise_shared::chaincode::chaincode_client::ChaincodeClient;
use raise_shared::chaincode::{chaincode_message, ChaincodeMessage}; // On importe le module interne pour l'Enum
use serde_json::Value;

// Adresse du conteneur Docker exposé sur l'hôte
const CHAINCODE_URL: &str = "http://127.0.0.1:9999";

// ==================================================================================
// LOGIQUE PURE (Testable unitairement)
// ==================================================================================

/// Construit le message Protobuf prêt à l'envoi.
fn build_register_request(
    uri: &str,
    version: &str,
    schema_json: &str,
) -> Result<ChaincodeMessage, String> {
    // Validation du JSON d'entrée
    let schema_def: Value =
        serde_json::from_str(schema_json).map_err(|e| format!("JSON du schéma invalide: {}", e))?;

    let args = serde_json::json!({
        "uri": uri,
        "version": version,
        "schema_definition": schema_def,
        "hash": "hash_simulé_tauri"
    });

    let payload = serde_json::to_vec(&serde_json::json!({
        "function": "register_schema",
        "args": args
    }))
    .map_err(|e| format!("Erreur de sérialisation interne: {}", e))?;

    let tx_id = uuid::Uuid::new_v4().to_string();

    Ok(ChaincodeMessage {
        // 2. Correction du chemin vers l'Enum Type
        r#type: chaincode_message::Type::Transaction as i32,
        timestamp_seconds: 0,
        payload,
        txid: tx_id,
    })
}

// ==================================================================================
// COMMANDE TAURI (Couche Réseau)
// ==================================================================================

#[tauri::command]
pub async fn cmd_register_schema(
    uri: String,
    version: String,
    schema_json: String,
) -> Result<String, String> {
    println!("🔌 [Tauri] Préparation de la transaction pour {}...", uri);

    let message = build_register_request(&uri, &version, &schema_json)?;
    let tx_id = message.txid.clone();

    println!(
        "🔌 [Tauri] Connexion au Chaincode Docker ({})...",
        CHAINCODE_URL
    );
    let mut client = ChaincodeClient::connect(CHAINCODE_URL).await.map_err(|e| {
        format!(
            "Échec connexion gRPC: {}. Avez-vous lancé 'docker-compose up' ?",
            e
        )
    })?;

    let request = tonic::Request::new(tokio_stream::iter(vec![message]));

    println!("📤 [Tauri] Envoi transaction TXID: {}", tx_id);

    // 3. Correction : on utilise .chat() au lieu de .connect()
    let mut stream = client
        .chat(request)
        .await
        .map_err(|e| format!("Erreur transport gRPC: {}", e))?
        .into_inner();

    // Lecture de la réponse (nécessite d'importer StreamExt ou message() selon la version)
    // Ici on utilise .message() qui est standard sur le stream retourné par tonic
    if let Some(response) = stream.message().await.map_err(|e| e.to_string())? {
        if response.r#type == chaincode_message::Type::Completed as i32 {
            println!("✅ [Tauri] Succès !");
            Ok(format!("Transaction validée ! TXID: {}", response.txid))
        } else {
            let err_msg = String::from_utf8_lossy(&response.payload);
            println!("❌ [Tauri] Erreur métier : {}", err_msg);
            Err(format!("Rejet Blockchain: {}", err_msg))
        }
    } else {
        Err("Aucune réponse reçue du conteneur".to_string())
    }
}

// ==================================================================================
// TESTS UNITAIRES
// ==================================================================================
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_request_success() {
        let uri = "schema:test_unit";
        let version = "1.0";
        let json_input = r#"{ "author": "Tester", "fields": ["a", "b"] }"#;

        let result = build_register_request(uri, version, json_input);

        assert!(result.is_ok());
        let msg = result.unwrap();

        assert_eq!(msg.r#type, chaincode_message::Type::Transaction as i32);
        assert!(!msg.txid.is_empty());
    }

    #[test]
    fn test_build_request_invalid_json() {
        let uri = "schema:bad";
        let version = "1.0";
        let bad_json = r#"{ "author": "Oubli de fermeture"#;

        let result = build_register_request(uri, version, bad_json);
        assert!(result.is_err());
    }
}
