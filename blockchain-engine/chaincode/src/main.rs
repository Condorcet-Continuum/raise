// blockchain-engine/chaincode/src/main.rs
// ==================================================================================
// ARCHITECTURE: RAISE CaaS
// ----------------------------------------------------------------------------------
// Point d'entrée principal.
// Il assemble :
// 1. Le Serveur gRPC (chaincode.rs)
// 2. La Mémoire (ledger.rs - Mock pour l'instant)
// 3. Le Cerveau (contract.rs)
// ==================================================================================

mod chaincode;
mod contract;
mod ledger;

use chaincode::chaincode_server::{Chaincode, ChaincodeServer};
use chaincode::ChaincodeMessage;
use ledger::{LedgerContext, MockLedger};
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tokio_stream::{Stream, StreamExt};
use tonic::{transport::Server, Request, Response, Status};

// Notre structure principale contient le contexte du Ledger
pub struct MyChaincode {
    ledger_ctx: Arc<LedgerContext>,
}

// --- CORRECTIF CLIPPY : Implémentation de Default ---
impl Default for MyChaincode {
    fn default() -> Self {
        Self::new()
    }
}

impl MyChaincode {
    /// Constructeur : Initialise le Ledger (Mock pour l'instant)
    pub fn new() -> Self {
        // Dans le futur, on détectera si on est en PROD (gRPC Ledger) ou DEV (Mock)
        let ledger_service = Arc::new(MockLedger::new());
        Self {
            ledger_ctx: Arc::new(LedgerContext::new(ledger_service)),
        }
    }

    /// CŒUR DU TRAITEMENT (Générique pour Prod et Tests)
    async fn handle_connection<S>(
        &self,
        mut in_stream: S,
    ) -> mpsc::Receiver<Result<ChaincodeMessage, Status>>
    where
        S: Stream<Item = Result<ChaincodeMessage, Status>> + Unpin + Send + 'static,
    {
        let (tx, rx) = mpsc::channel(128);

        // On clone la référence au ledger pour la passer à la tâche asynchrone
        let ledger = self.ledger_ctx.clone();

        tokio::spawn(async move {
            while let Some(result) = in_stream.next().await {
                match result {
                    Ok(msg) => {
                        // On passe le message ET le ledger au contrat
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
    type ConnectStream = ReceiverStream<Result<ChaincodeMessage, Status>>;

    // Point d'entrée gRPC (Production)
    async fn connect(
        &self,
        request: Request<tonic::Streaming<ChaincodeMessage>>,
    ) -> Result<Response<Self::ConnectStream>, Status> {
        let in_stream = request.into_inner();
        let rx = self.handle_connection(in_stream).await;
        Ok(Response::new(ReceiverStream::new(rx)))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = "0.0.0.0:9999".parse()?;

    // Initialisation du Chaincode avec son Ledger via new() (ou default())
    let cc = MyChaincode::default();

    println!("RAISE Chaincode server listening on {}", addr);

    Server::builder()
        .add_service(ChaincodeServer::new(cc))
        .serve(addr)
        .await?;

    Ok(())
}

// ==================================================================================
// TESTS D'INTÉGRATION
// ==================================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use chaincode::chaincode_message::Type;
    use serde_json::json;

    #[tokio::test]
    async fn test_full_integration_register_schema() {
        // 1. Initialiser le Chaincode complet
        let service = MyChaincode::new();

        // 2. Simuler le flux réseau
        let (tx_in, rx_in) = mpsc::channel(1);
        let input_stream = ReceiverStream::new(rx_in);

        // 3. Créer un message RÉEL de transaction (Register Schema)
        let args = json!({
            "uri": "schema:test_integration",
            "version": "1.0",
            "schema_definition": {},
            "hash": "123"
        });
        let payload = serde_json::to_vec(&json!({
            "function": "register_schema",
            "args": args
        }))
        .unwrap();

        let msg = ChaincodeMessage {
            r#type: Type::Transaction as i32,
            payload,
            txid: "tx_int_1".to_string(),
            timestamp_seconds: 0,
        };

        // 4. Envoyer
        tokio::spawn(async move {
            tx_in.send(Ok(msg)).await.unwrap();
        });

        // 5. Traiter
        let rx_out = service.handle_connection(input_stream).await;
        let mut out_stream = ReceiverStream::new(rx_out);

        // 6. Vérifier
        if let Some(result) = out_stream.next().await {
            let reply = result.expect("Should receive reply");

            // Vérifier que le contrat a répondu "Completed"
            assert_eq!(reply.r#type, Type::Completed as i32);
            assert_eq!(reply.txid, "tx_int_1");

            // 7. VÉRIFICATION ULTIME : Le ledger a-t-il été mis à jour ?
            let stored = service
                .ledger_ctx
                .exists("schema:test_integration")
                .await
                .unwrap();
            assert!(stored, "The schema should be persisted in the MockLedger");
        } else {
            panic!("No response received");
        }
    }
}
