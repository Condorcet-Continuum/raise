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
) -> RaiseResult<ChaincodeMessage, String> {
    // Validation du JSON d'entrée
    let schema_def: Value = match serde_json::from_str(schema_json) {
        Ok(val) => val,
        Err(e) => raise_error!(
            "ERR_JSONDB_SCHEMA_SYNTAX",
            error = e,
            context = json!({
                "action": "parse_schema_definition",
                "line": e.line(),
                "column": e.column(),
                "hint": "Le JSON du schéma est mal formé. Vérifiez les accolades et les guillemets."
            })
        ),
    };

    let args = serde_json::json!({
        "uri": uri,
        "version": version,
        "schema_definition": schema_def,
        "hash": "hash_simulé_tauri"
    });

    let payload = match serde_json::to_vec(&serde_json::json!({
        "function": "register_schema",
        "args": args
    })) {
        Ok(bytes) => bytes,
        Err(e) => raise_error!(
            "ERR_SERIALIZATION_PAYLOAD_FAILED",
            error = e,
            context = json!({
                "action": "serialize_schema_payload",
                "function": "register_schema",
                "hint": "Vérifiez que les arguments ne contiennent pas de types non supportés ou de références circulaires."
            })
        ),
    };

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
) -> RaiseResult<String, String> {
    println!("🔌 [Tauri] Préparation de la transaction pour {}...", uri);

    let message = build_register_request(&uri, &version, &schema_json)?;
    let tx_id = message.txid.clone();

    println!(
        "🔌 [Tauri] Connexion au Chaincode Docker ({})...",
        CHAINCODE_URL
    );
    let mut client = match ChaincodeClient::connect(CHAINCODE_URL).await {
        Ok(c) => c,
        Err(e) => {
            raise_error!(
                "ERR_BLOCKCHAIN_GRPC_CONNECTION_FAILED",
                error = e,
                context = json!({
                    "url": CHAINCODE_URL,
                    "protocol": "gRPC",
                    "hint": "Échec de la connexion au Chaincode. Avez-vous lancé 'docker-compose up' ? Vérifiez que le service est accessible sur le port configuré."
                })
            )
        }
    };

    let request = tonic::Request::new(tokio_stream::iter(vec![message]));

    println!("📤 [Tauri] Envoi transaction TXID: {}", tx_id);

    // 3. Correction : on utilise .chat() au lieu de .connect()
    let mut stream = match client.chat(request).await {
        Ok(response) => response.into_inner(),
        Err(e) => raise_error!(
            "ERR_GRPC_TRANSPORT_FAILURE",
            error = e,
            context = json!({
                "action": "establish_grpc_stream",
                "service": "chat_service",
                "hint": "Vérifiez la connexion réseau ou si le serveur distant est bien en ligne."
            })
        ),
    };

    // Lecture de la réponse (nécessite d'importer StreamExt ou message() selon la version)
    // Ici on utilise .message() qui est standard sur le stream retourné par tonic
    // 1. Lecture sécurisée du message depuis le stream gRPC
    let response = match stream.message().await {
        Ok(Some(msg)) => msg,
        Ok(None) => return raise_error!(
            "ERR_BLOCKCHAIN_EMPTY_RESPONSE",
            context = json!({ "hint": "Le conteneur a fermé la connexion sans envoyer de réponse." })
        ),
        Err(e) => return raise_error!(
            "ERR_BLOCKCHAIN_STREAM_FAILED",
            error = e,
            context = json!({ "protocol": "gRPC_Stream" })
        ),
    };

    // 2. Traitement de la réponse métier
    if response.r#type == chaincode_message::Type::Completed as i32 {
        println!("✅ [Tauri] Succès !");
        Ok(format!("Transaction validée ! TXID: {}", response.txid))
    } else {
        let err_msg = String::from_utf8_lossy(&response.payload).to_string();
        
        // On lève une erreur métier structurée
        raise_error!(
            "ERR_BLOCKCHAIN_TRANSACTION_REJECTED",
            context = json!({
                "txid": response.txid,
                "payload_error": err_msg,
                "type": response.r#type
            })
        )
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
