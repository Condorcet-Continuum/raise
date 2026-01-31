use raise_chaincode::chaincode::chaincode_client::ChaincodeClient;
use raise_chaincode::chaincode::chaincode_message::Type;
use raise_chaincode::chaincode::ChaincodeMessage;
use serde_json::json;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tonic::Request;

// ==================================================================================
// LOGIQUE MÉTIER (Extraite pour être testable sans réseau)
// ==================================================================================

/// Construit le message gRPC pour une transaction "register_schema"
fn build_register_transaction(uri: &str, version: &str, txid: &str) -> ChaincodeMessage {
    let args = json!({
        "uri": uri,
        "version": version,
        "schema_definition": { "author": "Client CLI", "test_mode": true },
        "hash": format!("hash_{}", uri)
    });

    let payload = serde_json::to_vec(&json!({
        "function": "register_schema",
        "args": args
    }))
    .unwrap();

    ChaincodeMessage {
        r#type: Type::Transaction as i32,
        timestamp_seconds: 123456, // Timestamp fixe ou actuel
        payload,
        txid: txid.to_string(),
    }
}

/// Analyse la réponse brute du Chaincode et retourne un diagnostic lisible
fn analyze_response(msg: &ChaincodeMessage) -> Result<String, String> {
    if msg.r#type == Type::Completed as i32 {
        Ok(format!("SUCCÈS (TXID: {})", msg.txid))
    } else {
        let err_msg = String::from_utf8_lossy(&msg.payload).to_string();
        Err(format!("ERREUR CHAINCODE: {}", err_msg))
    }
}

// ==================================================================================
// MAIN (Point d'entrée - Couche Réseau)
// ==================================================================================

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔌 Tentative de connexion au Chaincode sur http://127.0.0.1:9999 ...");

    // 1. Connexion Réseau
    let mut client = match ChaincodeClient::connect("http://127.0.0.1:9999").await {
        Ok(c) => c,
        Err(e) => {
            eprintln!("❌ Échec de connexion : {}", e);
            eprintln!("💡 Astuce : 'docker-compose --profile blockchain up'");
            return Ok(());
        }
    };
    println!("✅ Connecté !");

    // 2. Préparation du canal gRPC
    let (tx, rx) = mpsc::channel(4);
    let request = Request::new(ReceiverStream::new(rx));

    // 3. Construction du message (via notre fonction testable)
    let msg = build_register_transaction("schema:client_cli_test", "2.0", "tx_cli_001");

    // 4. Envoi
    tokio::spawn(async move {
        println!("📤 Envoi de la commande...");
        if let Err(e) = tx.send(msg).await {
            eprintln!("❌ Erreur d'envoi : {}", e);
        }
    });

    // 5. Réception et Analyse
    println!("⏳ Attente de la réponse...");
    let mut response_stream = client.connect(request).await?.into_inner();

    if let Some(received) = response_stream.message().await? {
        match analyze_response(&received) {
            Ok(success_msg) => println!("✅ {}", success_msg),
            Err(error_msg) => println!("❌ {}", error_msg),
        }
    } else {
        println!("⚠️ Connexion fermée sans réponse.");
    }

    Ok(())
}

// ==================================================================================
// TESTS UNITAIRES (Validation de la logique client)
// ==================================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use raise_chaincode::contract::ContractInput; // On réutilise les types de la lib pour vérifier

    // Teste que la fonction de construction génère un JSON valide
    #[test]
    fn test_build_transaction_structure() {
        let msg = build_register_transaction("schema:test_unit", "1.5", "tx_99");

        assert_eq!(msg.txid, "tx_99");
        assert_eq!(msg.r#type, Type::Transaction as i32);

        // On décode le payload pour vérifier son contenu
        let input: ContractInput = serde_json::from_slice(&msg.payload).unwrap();

        assert_eq!(input.function, "register_schema");
        assert_eq!(input.args["uri"], "schema:test_unit");
        assert_eq!(input.args["version"], "1.5");
    }

    // Teste l'analyse d'une réponse positive
    #[test]
    fn test_analyze_response_success() {
        let response = ChaincodeMessage {
            r#type: Type::Completed as i32,
            payload: b"OK".to_vec(),
            txid: "tx_ok".to_string(),
            timestamp_seconds: 0,
        };

        let result = analyze_response(&response);
        assert!(result.is_ok());
        assert!(result.unwrap().contains("SUCCÈS"));
    }

    // Teste l'analyse d'une réponse d'erreur
    #[test]
    fn test_analyze_response_error() {
        let response = ChaincodeMessage {
            r#type: Type::Error as i32,
            payload: b"Schema already exists".to_vec(),
            txid: "tx_fail".to_string(),
            timestamp_seconds: 0,
        };

        let result = analyze_response(&response);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            "ERREUR CHAINCODE: Schema already exists"
        );
    }
}
