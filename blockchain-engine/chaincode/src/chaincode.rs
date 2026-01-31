// blockchain-engine/chaincode/src/chaincode.rs
// ==================================================================================
// ARCHITECTURE: RAISE CaaS (Chaincode-as-a-Service)
// ----------------------------------------------------------------------------------
// VERSION FINALE : Corrigée pour clippy (E0308 - type mismatch)
// ==================================================================================

#![allow(unused_variables, dead_code, missing_docs, clippy::let_unit_value)]

use http_body_util::BodyExt;
use std::marker::PhantomData;
use tonic::codegen::*;

// ==================================================================================
// 1. TYPES HTTP MAISON
// ==================================================================================
pub type BoxBody = http_body_util::combinators::UnsyncBoxBody<bytes::Bytes, tonic::Status>;

pub fn empty_body() -> BoxBody {
    http_body_util::Empty::new()
        .map_err(|_| tonic::Status::internal("should not happen"))
        .boxed_unsync()
}

// ==================================================================================
// 2. MESSAGES PROTOBUF
// ==================================================================================
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ChaincodeMessage {
    #[prost(enumeration = "chaincode_message::Type", tag = "1")]
    pub r#type: i32,
    #[prost(int64, tag = "2")]
    pub timestamp_seconds: i64,
    #[prost(bytes = "vec", tag = "3")]
    pub payload: ::prost::alloc::vec::Vec<u8>,
    #[prost(string, tag = "4")]
    pub txid: ::prost::alloc::string::String,
}

pub mod chaincode_message {
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
    #[repr(i32)]
    pub enum Type {
        Undefined = 0,
        Register = 1,
        Registered = 2,
        Init = 3,
        Ready = 4,
        Transaction = 5,
        Completed = 6,
        Error = 7,
        Response = 13,
    }
}

// ==================================================================================
// 3. CODEC
// ==================================================================================
#[derive(Debug, Clone)]
pub struct MyProstCodec<T, U>(PhantomData<(T, U)>);

impl<T, U> Default for MyProstCodec<T, U> {
    fn default() -> Self {
        Self(PhantomData)
    }
}

impl<T, U> tonic::codec::Codec for MyProstCodec<T, U>
where
    T: prost::Message + Send + 'static,
    U: prost::Message + Default + Send + 'static,
{
    type Encode = T;
    type Decode = U;
    type Encoder = MyProstEncoder<T>;
    type Decoder = MyProstDecoder<U>;

    fn encoder(&mut self) -> Self::Encoder {
        MyProstEncoder(PhantomData)
    }
    fn decoder(&mut self) -> Self::Decoder {
        MyProstDecoder(PhantomData)
    }
}

pub struct MyProstEncoder<T>(PhantomData<T>);
impl<T: prost::Message> tonic::codec::Encoder for MyProstEncoder<T> {
    type Item = T;
    type Error = tonic::Status;
    fn encode(
        &mut self,
        item: T,
        dst: &mut tonic::codec::EncodeBuf<'_>,
    ) -> Result<(), Self::Error> {
        item.encode(dst)
            .map_err(|e| tonic::Status::internal(e.to_string()))
    }
}

pub struct MyProstDecoder<U>(PhantomData<U>);
impl<U: prost::Message + Default> tonic::codec::Decoder for MyProstDecoder<U> {
    type Item = U;
    type Error = tonic::Status;
    fn decode(&mut self, src: &mut tonic::codec::DecodeBuf<'_>) -> Result<Option<U>, Self::Error> {
        U::decode(src)
            .map(Some)
            .map_err(|e| tonic::Status::internal(e.to_string()))
    }
}

// ==================================================================================
// 4. SERVEUR gRPC
// ==================================================================================
pub mod chaincode_server {
    use super::*;

    #[async_trait]
    pub trait Chaincode: Send + Sync + 'static {
        type ConnectStream: tokio_stream::Stream<Item = Result<super::ChaincodeMessage, tonic::Status>>
            + Send
            + 'static;
        async fn connect(
            &self,
            request: tonic::Request<tonic::Streaming<super::ChaincodeMessage>>,
        ) -> Result<tonic::Response<Self::ConnectStream>, tonic::Status>;
    }

    #[derive(Debug)]
    pub struct ChaincodeServer<T: Chaincode> {
        inner: Arc<T>,
        accept_compression_encodings: EnabledCompressionEncodings,
        send_compression_encodings: EnabledCompressionEncodings,
        max_decoding_message_size: Option<usize>,
        max_encoding_message_size: Option<usize>,
    }

    impl<T: Chaincode> ChaincodeServer<T> {
        pub fn new(inner: T) -> Self {
            Self::from_arc(Arc::new(inner))
        }
        pub fn from_arc(inner: Arc<T>) -> Self {
            Self {
                inner,
                accept_compression_encodings: Default::default(),
                send_compression_encodings: Default::default(),
                max_decoding_message_size: None,
                max_encoding_message_size: None,
            }
        }
    }

    impl<T: Chaincode> Clone for ChaincodeServer<T> {
        fn clone(&self) -> Self {
            Self {
                inner: self.inner.clone(),
                accept_compression_encodings: self.accept_compression_encodings,
                send_compression_encodings: self.send_compression_encodings,
                max_decoding_message_size: self.max_decoding_message_size,
                max_encoding_message_size: self.max_encoding_message_size,
            }
        }
    }

    impl<T: Chaincode> tonic::server::NamedService for ChaincodeServer<T> {
        const NAME: &'static str = "protos.Chaincode";
    }

    impl<T, B> tonic::codegen::Service<http::Request<B>> for ChaincodeServer<T>
    where
        T: Chaincode,
        B: Body + Send + 'static,
        B::Error: Into<StdError> + Send + 'static,
    {
        type Response = http::Response<super::BoxBody>;
        type Error = std::convert::Infallible;
        type Future = BoxFuture<Self::Response, Self::Error>;

        fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }

        fn call(&mut self, req: http::Request<B>) -> Self::Future {
            let inner = self.inner.clone();

            // Capture des configs pour le bloc async
            let accept_compression_encodings = self.accept_compression_encodings;
            let send_compression_encodings = self.send_compression_encodings;
            let max_decoding_message_size = self.max_decoding_message_size;
            let max_encoding_message_size = self.max_encoding_message_size;

            match req.uri().path() {
                "/protos.Chaincode/Connect" => {
                    Box::pin(async move {
                        struct ConnectSvc<T: Chaincode>(pub Arc<T>);
                        impl<T: Chaincode> tonic::server::StreamingService<super::ChaincodeMessage> for ConnectSvc<T> {
                            type Response = super::ChaincodeMessage;
                            type ResponseStream = T::ConnectStream;
                            type Future =
                                BoxFuture<tonic::Response<Self::ResponseStream>, tonic::Status>;
                            fn call(
                                &mut self,
                                request: tonic::Request<tonic::Streaming<super::ChaincodeMessage>>,
                            ) -> Self::Future {
                                let inner = self.0.clone();
                                let fut =
                                    async move { <T as Chaincode>::connect(&inner, request).await };
                                Box::pin(fut)
                            }
                        }

                        let codec = super::MyProstCodec::<
                            super::ChaincodeMessage,
                            super::ChaincodeMessage,
                        >::default();

                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            )
                            .apply_max_message_size_config(
                                max_decoding_message_size,
                                max_encoding_message_size,
                            );

                        // ICI : On récupère la réponse directement (pas de Result)
                        let response = grpc.streaming(ConnectSvc(inner), req).await;

                        // On démonte et remonte le corps
                        let (parts, body) = response.into_parts();
                        let new_body = body.map_err(|e| e).boxed_unsync();

                        Ok(http::Response::from_parts(parts, new_body))
                    })
                }
                _ => Box::pin(async move {
                    Ok(http::Response::builder()
                        .status(200)
                        .header("grpc-status", "12")
                        .header("content-type", "application/grpc")
                        .body(super::empty_body())
                        .unwrap())
                }),
            }
        }
    }
}

// ==================================================================================
// 5. TESTS
// ==================================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use prost::Message;

    #[test]
    fn test_chaincode_message_structure() {
        let msg = ChaincodeMessage {
            r#type: chaincode_message::Type::Register as i32,
            timestamp_seconds: 1625097600,
            payload: b"test_payload".to_vec(),
            txid: "tx_12345".to_string(),
        };
        assert_eq!(msg.r#type, 1);
    }

    #[test]
    fn test_prost_serialization() {
        let original_msg = ChaincodeMessage {
            r#type: chaincode_message::Type::Transaction as i32,
            timestamp_seconds: 100,
            payload: vec![1, 2, 3, 4],
            txid: "abc".to_string(),
        };
        let mut buf = Vec::new();
        original_msg.encode(&mut buf).expect("Encoding failed");
        let decoded_msg = ChaincodeMessage::decode(&buf[..]).expect("Decoding failed");
        assert_eq!(decoded_msg, original_msg);
    }
}
