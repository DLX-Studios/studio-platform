//! Deterministic coverage for the host HTTPS transport boundary.

use std::sync::Arc;
use std::time::Duration;

use studio_net::declaration::HttpMethod;
use studio_net::transport::{
    ByteStream, HttpsClient, HttpTransport, IncomingResponse, OutgoingRequest,
    ProductionHttpTransport, TransportError, TransportLimits,
};

fn request(url: &str) -> OutgoingRequest {
    OutgoingRequest {
        method: HttpMethod::Get,
        url: url.to_owned(),
        headers: Vec::new(),
        body: None,
        timeout: Duration::from_secs(5),
    }
}

struct FakeClient {
    body: Vec<u8>,
}

impl HttpsClient for FakeClient {
    fn execute(
        &self,
        _request: OutgoingRequest,
        _limits: TransportLimits,
    ) -> Result<IncomingResponse, TransportError> {
        Ok(IncomingResponse {
            status: 200,
            media_type: Some("application/json".to_owned()),
            body: self.body.clone(),
        })
    }

    fn open_stream(
        &self,
        _request: OutgoingRequest,
        _limits: TransportLimits,
    ) -> Result<Box<dyn ByteStream>, TransportError> {
        Ok(Box::new(OneChunkStream {
            chunk: Some(self.body.clone()),
        }))
    }
}

struct OneChunkStream {
    chunk: Option<Vec<u8>>,
}

impl ByteStream for OneChunkStream {
    fn read_chunk(&mut self) -> Result<Option<Vec<u8>>, TransportError> {
        Ok(self.chunk.take())
    }
}

#[test]
fn production_transport_rejects_plaintext_before_client_invocation() {
    let transport = ProductionHttpTransport::new(
        Arc::new(FakeClient { body: b"ok".to_vec() }),
        TransportLimits::default(),
    );
    assert!(matches!(
        transport.execute(request("http://api.example.test/items")),
        Err(TransportError::ConnectionFailure)
    ));
}

#[test]
fn production_transport_enforces_response_body_limit() {
    let transport = ProductionHttpTransport::new(
        Arc::new(FakeClient {
            body: b"oversized".to_vec(),
        }),
        TransportLimits {
            max_response_bytes: 4,
            ..TransportLimits::default()
        },
    );
    assert!(matches!(
        transport.execute(request("https://api.example.test/items")),
        Err(TransportError::BodyTooLarge)
    ));
}

#[test]
fn production_transport_enforces_total_stream_bytes_and_chunk_size() {
    let transport = ProductionHttpTransport::new(
        Arc::new(FakeClient {
            body: b"chunk".to_vec(),
        }),
        TransportLimits {
            max_stream_bytes: 4,
            max_stream_chunk_bytes: 32,
            ..TransportLimits::default()
        },
    );
    let mut stream = transport
        .open_stream(request("https://api.example.test/events"))
        .expect("HTTPS stream opens");
    assert_eq!(stream.read_chunk(), Err(TransportError::BodyTooLarge));
}

#[test]
fn outgoing_request_debug_redacts_header_and_body_values() {
    let mut outgoing = request("https://api.example.test/items");
    outgoing.headers.push((
        "authorization".to_owned(),
        "Bearer do-not-print".to_owned(),
    ));
    outgoing.body = Some(b"also-do-not-print".to_vec());
    let rendered = format!("{outgoing:?}");
    assert!(!rendered.contains("do-not-print"));
    assert!(rendered.contains("authorization"));
    assert!(rendered.contains("body_bytes"));
}
