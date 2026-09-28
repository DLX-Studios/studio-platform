//! Operator-gated staging coverage through the same broker interface used by host integrations.

#![cfg(feature = "integration-websocket-real")]
#![allow(missing_docs, clippy::all, clippy::pedantic, dead_code)]

use std::io;
use std::net::TcpStream;
use std::time::Duration;

use serde_json::{Value, json};
use studio_net::websocket::{
    WebSocketBroker, WebSocketBrokerLimits, WebSocketConnectRequest, WebSocketConnection,
    WebSocketDeclaration, WebSocketDeclaredLimits, WebSocketEvent, WebSocketOpenRequest,
    WebSocketTransport, WebSocketTransportError,
};
use tungstenite::client::IntoClientRequest;
use tungstenite::http::header::{HeaderValue, SEC_WEBSOCKET_PROTOCOL};
use tungstenite::protocol::{Message, WebSocketConfig};
use tungstenite::stream::MaybeTlsStream;

const ENDPOINT_ENV: &str = "STUDIO_NET_REAL_WEBSOCKET_URL";
const MESSAGE: &str = "studio-net-websocket-gate";
const MAX_MESSAGE_BYTES: usize = 1024;
const SESSION_TIMEOUT: Duration = Duration::from_secs(30);

type Socket = tungstenite::WebSocket<MaybeTlsStream<TcpStream>>;

struct StagingTransport;

impl WebSocketTransport for StagingTransport {
    fn connect(
        &self,
        request: WebSocketConnectRequest,
    ) -> Result<Box<dyn WebSocketConnection>, WebSocketTransportError> {
        let mut handshake = request
            .endpoint
            .into_client_request()
            .map_err(|_| WebSocketTransportError::ConnectionFailure)?;
        if let Some(protocol) = request.subprotocol {
            let protocol = HeaderValue::from_str(&protocol)
                .map_err(|_| WebSocketTransportError::ConnectionFailure)?;
            handshake
                .headers_mut()
                .insert(SEC_WEBSOCKET_PROTOCOL, protocol);
        }

        let config = WebSocketConfig::default()
            .max_message_size(Some(MAX_MESSAGE_BYTES))
            .max_frame_size(Some(MAX_MESSAGE_BYTES));
        let (mut socket, _) = tungstenite::client::connect_with_config(handshake, Some(config), 0)
            .map_err(map_error)?;
        set_socket_timeouts(socket.get_mut(), request.timeout)?;
        Ok(Box::new(StagingConnection(socket)))
    }
}

struct StagingConnection(Socket);

impl WebSocketConnection for StagingConnection {
    fn send(&mut self, message: &[u8]) -> Result<(), WebSocketTransportError> {
        let text =
            std::str::from_utf8(message).map_err(|_| WebSocketTransportError::ConnectionFailure)?;
        self.0.send(Message::text(text)).map_err(map_error)
    }

    fn receive(&mut self) -> Result<Option<Vec<u8>>, WebSocketTransportError> {
        loop {
            match self.0.read() {
                Ok(message @ Message::Text(_)) | Ok(message @ Message::Binary(_)) => {
                    return Ok(Some(message.into_data().to_vec()));
                }
                Ok(Message::Close(_)) => return Ok(None),
                Ok(Message::Ping(_) | Message::Pong(_) | Message::Frame(_)) => {}
                Err(tungstenite::Error::ConnectionClosed | tungstenite::Error::AlreadyClosed) => {
                    return Ok(None);
                }
                Err(error) => return Err(map_error(error)),
            }
        }
    }

    fn close(&mut self) {
        let _ = self.0.close(None);
    }
}

fn set_socket_timeouts(
    stream: &mut MaybeTlsStream<TcpStream>,
    timeout: Duration,
) -> Result<(), WebSocketTransportError> {
    let socket = match stream {
        MaybeTlsStream::Plain(socket) => socket,
        MaybeTlsStream::Rustls(stream) => &mut stream.sock,
        _ => return Err(WebSocketTransportError::ConnectionFailure),
    };
    socket
        .set_read_timeout(Some(timeout))
        .and_then(|()| socket.set_write_timeout(Some(timeout)))
        .map_err(|error| match error.kind() {
            io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock => {
                WebSocketTransportError::TimedOut
            }
            _ => WebSocketTransportError::ConnectionFailure,
        })
}

fn map_error(error: tungstenite::Error) -> WebSocketTransportError {
    match error {
        tungstenite::Error::Capacity(_) => WebSocketTransportError::MessageTooLarge,
        tungstenite::Error::Io(error)
            if matches!(
                error.kind(),
                io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock
            ) =>
        {
            WebSocketTransportError::TimedOut
        }
        _ => WebSocketTransportError::ConnectionFailure,
    }
}

fn schema() -> Value {
    json!({
        "type": "object",
        "properties": {"message": {"type": "string"}},
        "required": ["message"],
        "additionalProperties": false
    })
}

#[test]
fn approved_staging_endpoint_echoes_through_host_owned_broker() {
    let endpoint = std::env::var(ENDPOINT_ENV)
        .unwrap_or_else(|_| panic!("{ENDPOINT_ENV} must be set for this staging gate"));
    assert!(
        endpoint.starts_with("wss://"),
        "{ENDPOINT_ENV} must use certificate-validated wss://"
    );

    let mut broker = WebSocketBroker::try_new(
        std::sync::Arc::new(StagingTransport),
        WebSocketBrokerLimits::default(),
    )
    .expect("valid host WebSocket limits");
    broker
        .declare(&WebSocketDeclaration {
            id: String::from("staging-echo"),
            endpoint: endpoint.clone(),
            subprotocols: Vec::new(),
            inbound_schema: schema(),
            outbound_schema: schema(),
            limits: WebSocketDeclaredLimits {
                max_message_bytes: Some(MAX_MESSAGE_BYTES),
                max_messages_per_window: Some(4),
                max_session_duration_ms: Some(
                    u64::try_from(SESSION_TIMEOUT.as_millis()).expect("timeout fits u64"),
                ),
                max_reconnects: Some(0),
                ..Default::default()
            },
        })
        .expect("staging declaration must satisfy host policy");

    let session = std::sync::Arc::new(broker)
        .guest_api()
        .open(WebSocketOpenRequest::new(endpoint))
        .expect("approved endpoint must open through the broker");
    assert!(matches!(
        session.next_event(),
        Some(WebSocketEvent::Opened { .. })
    ));

    let message = json!({"message": MESSAGE});
    session
        .send(message.clone())
        .expect("outbound schema is admitted");
    assert!(matches!(
        session.next_event(),
        Some(WebSocketEvent::Message(received)) if received == message
    ));
    assert!(matches!(session.next_event(), Some(WebSocketEvent::Closed)));
}
