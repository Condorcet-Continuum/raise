// blockchain-engine/chaincode/src/lib.rs

// 1. On retire 'pub mod chaincode' car le code est maintenant dans raise-shared
// pub mod chaincode; <--- Supprimé
pub mod contract;
pub mod ledger;

// 2. Imports depuis la librairie partagée (Source de vérité)
use raise_shared::chaincode::chaincode_server::Chaincode;
use raise_shared::chaincode::ChaincodeMessage;
// On a besoin de ce module pour accéder à l'Enum 'Type' dans les tests
use raise_shared::chaincode::chaincode_message;

use ledger::{LedgerContext, MockLedger};
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tokio_stream::{Stream, StreamExt};
use tonic::{Request, Response, Status};

// Structure publique pour être utilisée par le serveur et les tests
pub struct MyChaincode {
    pub ledger_ctx: Arc<LedgerContext>,
}

impl Default for MyChaincode {
    fn default() -> Self {
        Self::new()
    }
}

impl MyChaincode {
    pub fn new() -> Self {
        let ledger_service = Arc::new(MockLedger::new());
        Self {
            ledger_ctx: Arc::new(LedgerContext::new(ledger_service)),
        }
    }

    /// Fonction de traitement interne (Testable sans réseau)
    /// Rendue publique ou accessible aux tests pour simulation
    pub async fn handle_connection<S>(
        &self,
        mut in_stream: S,
    ) -> mpsc::Receiver<Result<ChaincodeMessage, Status>>
    where
        S: Stream<Item = Result<ChaincodeMessage, Status>> + Unpin + Send + 'static,
    {
        let (tx, rx) = mpsc::channel(128);
        let ledger = self.ledger_ctx.clone();

        tokio::spawn(async move {
            while let Some(result) = in_stream.next().await {
                match result {
                    Ok(msg) => {
                        // Exécution du contrat
                        let response = contract::RaiseContract::execute(&ledger, &msg).await;

                        if let Err(e) = tx.send(Ok(response)).await {
                            eprintln!("Failed to send response: {}", e);
                            break;
                        }
                    }
                    Err(e) => {
                        eprintln!("Error receiving message: {}", e);
                        break;
                    }
                }
            }
        });

        rx
    }
}

#[tonic::async_trait]
impl Chaincode for MyChaincode {
    // CORRECTION : ConnectStream -> ChatStream (Défini par le .proto)
    type ChatStream = ReceiverStream<Result<ChaincodeMessage, Status>>;

    // CORRECTION : connect -> chat (Défini par le .proto)
    async fn chat(
        &self,
        request: Request<tonic::Streaming<ChaincodeMessage>>,
    ) -> Result<Response<Self::ChatStream>, Status> {
        let in_stream = request.into_inner();
        let rx = self.handle_connection(in_stream).await;
        Ok(Response::new(ReceiverStream::new(rx)))
    }
}

// ==================================================================================
// TESTS UNITAIRES ET D'INTÉGRATION
// ==================================================================================
#[cfg(test)]
mod tests {
    use super::*;
    // Mise à jour de l'import pour l'Enum Type
    use raise_shared::chaincode::chaincode_message::Type;
    use serde_json::json;

    // Helper pour simuler un message entrant
    fn create_msg(function: &str, args: serde_json::Value, txid: &str) -> ChaincodeMessage {
        let payload = serde_json::to_vec(&json!({
            "function": function,
            "args": args
        }))
        .unwrap();

        ChaincodeMessage {
            r#type: Type::Transaction as i32,
            timestamp_seconds: 0,
            payload,
            txid: txid.to_string(),
        }
    }

    #[tokio::test]
    async fn test_initialization() {
        let cc = MyChaincode::default();
        // Vérifie que le ledger est bien initialisé (vide mais existant)
        assert!(cc.ledger_ctx.exists("random_key").await.is_ok());
    }

    #[tokio::test]
    async fn test_full_workflow_register_schema() {
        // 1. Setup
        let service = MyChaincode::new();
        let (tx_in, rx_in) = mpsc::channel(10);
        let input_stream = ReceiverStream::new(rx_in);

        // 2. Création de la requête (Register Schema)
        let schema_args = json!({
            "uri": "schema:unit_test",
            "version": "1.0",
            "schema_definition": {"field": "value"},
            "hash": "abc"
        });
        let msg = create_msg("register_schema", schema_args, "tx_1");

        // 3. Envoi asynchrone
        tokio::spawn(async move {
            tx_in.send(Ok(msg)).await.unwrap();
        });

        // 4. Traitement
        let rx_out = service.handle_connection(input_stream).await;
        let mut out_stream = ReceiverStream::new(rx_out);

        // 5. Vérification de la réponse
        if let Some(result) = out_stream.next().await {
            let response = result.expect("Should contain a response");
            assert_eq!(response.r#type, Type::Completed as i32);
            assert_eq!(response.txid, "tx_1");

            // 6. Vérification de l'effet de bord sur le Ledger
            let exists = service.ledger_ctx.exists("schema:unit_test").await.unwrap();
            assert!(exists, "Schema should be saved in MockLedger");
        } else {
            panic!("Stream ended unexpectedly");
        }
    }

    #[tokio::test]
    async fn test_workflow_error_handling() {
        let service = MyChaincode::new();
        let (tx_in, rx_in) = mpsc::channel(1);
        let input_stream = ReceiverStream::new(rx_in);

        // Envoi d'une fonction inconnue
        let msg = create_msg("unknown_function", json!({}), "tx_err");

        tokio::spawn(async move {
            tx_in.send(Ok(msg)).await.unwrap();
        });

        let rx_out = service.handle_connection(input_stream).await;
        let mut out_stream = ReceiverStream::new(rx_out);

        if let Some(result) = out_stream.next().await {
            let response = result.expect("Response");
            assert_eq!(response.r#type, Type::Error as i32);
            let err_msg = String::from_utf8(response.payload).unwrap();
            assert!(err_msg.contains("Unknown function"));
        }
    }

    // NOUVEAU TEST : Vérifie spécifiquement le point d'entrée gRPC 'chat'
    #[tokio::test]
    async fn test_grpc_chat_entrypoint() {
        let service = MyChaincode::new();

        // On crée un stream vide juste pour voir si la méthode répond sans paniquer
        let input_stream = tokio_stream::iter(vec![]);
        let request = Request::new(input_stream);

        // On appelle la méthode RENOMMÉE
        let response = service.chat(request).await;

        assert!(
            response.is_ok(),
            "Le serveur doit accepter la connexion Chat"
        );
    }
}
