//! Deterministic host-owned WebSocket broker coverage using a local scripted transport seam.

#![allow(missing_docs)]
#![allow(clippy::type_complexity, clippy::default_trait_access)]

use std::collections::VecDeque;
use std::sync::{Arc, Mutex, mpsc};

use serde_json::{Value, json};
use studio_net::error::BrokerErrorCode;
use studio_net::websocket::{
    WebSocketBroker, WebSocketBrokerLimits, WebSocketConnection, WebSocketDeclaration,
    WebSocketEvent, WebSocketOpenRequest, WebSocketTransport, WebSocketTransportError,
};

struct ScriptedTransport {
    plans: Mutex<
        VecDeque<
            Result<
                VecDeque<Result<Option<Vec<u8>>, WebSocketTransportError>>,
                WebSocketTransportError,
            >,
        >,
    >,
}

impl ScriptedTransport {
    fn new(
        plans: Vec<
            Result<
                VecDeque<Result<Option<Vec<u8>>, WebSocketTransportError>>,
                WebSocketTransportError,
            >,
        >,
    ) -> Self {
        Self {
            plans: Mutex::new(plans.into()),
        }
    }
}

impl WebSocketTransport for ScriptedTransport {
    fn connect(
        &self,
        _request: studio_net::websocket::WebSocketConnectRequest,
    ) -> Result<Box<dyn WebSocketConnection>, WebSocketTransportError> {
        self.plans
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or(Err(WebSocketTransportError::ConnectionFailure))
            .map(|receives| {
                Box::new(ScriptedConnection { receives }) as Box<dyn WebSocketConnection>
            })
    }
}

struct ScriptedConnection {
    receives: VecDeque<Result<Option<Vec<u8>>, WebSocketTransportError>>,
}

impl WebSocketConnection for ScriptedConnection {
    fn send(&mut self, _message: &[u8]) -> Result<(), WebSocketTransportError> {
        Ok(())
    }

    fn receive(&mut self) -> Result<Option<Vec<u8>>, WebSocketTransportError> {
        self.receives.pop_front().unwrap_or(Ok(None))
    }

    fn close(&mut self) {}
}

fn declaration() -> WebSocketDeclaration {
    WebSocketDeclaration {
        id: String::from("events"),
        endpoint: String::from("ws://127.0.0.1:9000/events"),
        subprotocols: vec![String::from("studio.v1")],
        inbound_schema: json!({
            "type": "object",
            "properties": {"kind": {"type": "string"}},
            "required": ["kind"],
        }),
        outbound_schema: json!({
            "type": "object",
            "properties": {"kind": {"type": "string"}},
            "required": ["kind"],
        }),
        limits: Default::default(),
    }
}

fn broker(transport: Arc<dyn WebSocketTransport>) -> Arc<WebSocketBroker> {
    broker_with_limits(
        transport,
        WebSocketBrokerLimits {
            reconnect_base_delay: std::time::Duration::from_millis(1),
            ..Default::default()
        },
    )
}

fn broker_with_limits(
    transport: Arc<dyn WebSocketTransport>,
    limits: WebSocketBrokerLimits,
) -> Arc<WebSocketBroker> {
    let mut broker = WebSocketBroker::new(transport, limits);
    broker.declare(&declaration()).unwrap();
    Arc::new(broker)
}

struct GatedTransport {
    entered: mpsc::Sender<()>,
    release: Mutex<mpsc::Receiver<()>>,
}

impl WebSocketTransport for GatedTransport {
    fn connect(
        &self,
        _request: studio_net::websocket::WebSocketConnectRequest,
    ) -> Result<Box<dyn WebSocketConnection>, WebSocketTransportError> {
        self.entered
            .send(())
            .map_err(|_| WebSocketTransportError::ConnectionFailure)?;
        self.release
            .lock()
            .unwrap()
            .recv()
            .map_err(|_| WebSocketTransportError::ConnectionFailure)?;
        Ok(Box::new(ScriptedConnection {
            receives: VecDeque::new(),
        }))
    }
}

#[test]
fn admission_and_typed_lifecycle_are_host_owned() {
    let transport = Arc::new(ScriptedTransport::new(vec![Ok(VecDeque::from([
        Ok(Some(br#"{"kind":"ready"}"#.to_vec())),
        Ok(None),
    ]))]));
    let broker = broker(transport);
    let session = broker
        .guest_api()
        .open(WebSocketOpenRequest::new("ws://127.0.0.1:9000/events").with_subprotocol("studio.v1"))
        .unwrap();
    assert!(format!("{:?}", session.id()).contains("OpaqueSessionId(..)"));
    assert!(matches!(
        session.next_event(),
        Some(WebSocketEvent::Opened { .. })
    ));
    assert!(matches!(
        session.next_event(),
        Some(WebSocketEvent::Message(Value::Object(_)))
    ));
    assert!(matches!(session.next_event(), Some(WebSocketEvent::Closed)));
    assert!(session.next_event().is_none());
    assert_eq!(
        broker
            .guest_api()
            .open(WebSocketOpenRequest::new("ws://127.0.0.1:9000/other"))
            .unwrap_err()
            .code(),
        BrokerErrorCode::WebSocketEndpointNotDeclared
    );
}

#[test]
fn outbound_schema_and_subprotocol_admission_fail_before_connect() {
    let transport = Arc::new(ScriptedTransport::new(Vec::new()));
    let broker = broker(transport);
    let error = broker
        .guest_api()
        .open(WebSocketOpenRequest::new("ws://127.0.0.1:9000/events").with_subprotocol("wrong"))
        .unwrap_err();
    assert_eq!(
        error.code(),
        BrokerErrorCode::WebSocketSubprotocolNotAllowed
    );
}

#[test]
fn reconnect_is_scheduled_by_host_and_invalid_peer_data_is_not_delivered() {
    let transport = Arc::new(ScriptedTransport::new(vec![
        Err(WebSocketTransportError::ConnectionFailure),
        Ok(VecDeque::from([
            Ok(Some(br#"{"kind":"after-reconnect"}"#.to_vec())),
            Ok(None),
        ])),
    ]));
    let broker = broker(transport);
    let session = broker
        .guest_api()
        .open(WebSocketOpenRequest::new("ws://127.0.0.1:9000/events"))
        .unwrap();
    assert!(matches!(
        session.next_event(),
        Some(WebSocketEvent::Reconnecting { attempt: 1, .. })
    ));
    assert!(matches!(
        session.next_event(),
        Some(WebSocketEvent::Opened { .. })
    ));
    assert!(matches!(
        session.next_event(),
        Some(WebSocketEvent::Message(_))
    ));
    assert!(matches!(session.next_event(), Some(WebSocketEvent::Closed)));
}

#[test]
fn inbound_message_size_is_enforced_mid_session() {
    let transport = Arc::new(ScriptedTransport::new(vec![Ok(VecDeque::from([
        Ok(Some(br#"{"kind":"ok"}"#.to_vec())),
        Ok(Some(
            br#"{"kind":"this-message-exceeds-the-limit"}"#.to_vec(),
        )),
    ]))]));
    let broker = broker_with_limits(
        transport,
        WebSocketBrokerLimits {
            max_message_bytes: 24,
            ..Default::default()
        },
    );
    let session = broker
        .guest_api()
        .open(WebSocketOpenRequest::new("ws://127.0.0.1:9000/events"))
        .unwrap();

    assert!(matches!(
        session.next_event(),
        Some(WebSocketEvent::Opened { .. })
    ));
    assert!(matches!(
        session.next_event(),
        Some(WebSocketEvent::Message(_))
    ));
    assert!(matches!(
        session.next_event(),
        Some(WebSocketEvent::Error(error))
            if error.code() == BrokerErrorCode::WebSocketMessageTooLarge
    ));
    assert!(matches!(session.next_event(), Some(WebSocketEvent::Closed)));
}

#[test]
fn inbound_message_rate_is_enforced_mid_session() {
    let transport = Arc::new(ScriptedTransport::new(vec![Ok(VecDeque::from([
        Ok(Some(br#"{"kind":"first"}"#.to_vec())),
        Ok(Some(br#"{"kind":"second"}"#.to_vec())),
    ]))]));
    let broker = broker_with_limits(
        transport,
        WebSocketBrokerLimits {
            max_messages_per_window: 1,
            ..Default::default()
        },
    );
    let session = broker
        .guest_api()
        .open(WebSocketOpenRequest::new("ws://127.0.0.1:9000/events"))
        .unwrap();

    assert!(matches!(
        session.next_event(),
        Some(WebSocketEvent::Opened { .. })
    ));
    assert!(matches!(
        session.next_event(),
        Some(WebSocketEvent::Message(_))
    ));
    assert!(matches!(
        session.next_event(),
        Some(WebSocketEvent::Error(error))
            if error.code() == BrokerErrorCode::WebSocketRateLimited
    ));
    assert!(matches!(session.next_event(), Some(WebSocketEvent::Closed)));
}

#[test]
fn outbound_schema_size_and_rate_are_enforced_before_transport_send() {
    let (entered_sender, entered_receiver) = mpsc::channel();
    let (release_sender, release_receiver) = mpsc::channel();
    let transport = Arc::new(GatedTransport {
        entered: entered_sender,
        release: Mutex::new(release_receiver),
    });
    let broker = broker_with_limits(
        transport,
        WebSocketBrokerLimits {
            max_message_bytes: 24,
            max_messages_per_window: 1,
            ..Default::default()
        },
    );
    let session = broker
        .guest_api()
        .open(WebSocketOpenRequest::new("ws://127.0.0.1:9000/events"))
        .unwrap();
    entered_receiver
        .recv_timeout(std::time::Duration::from_secs(1))
        .unwrap();

    let invalid_schema = session.send(json!({"unexpected": "value"}));
    let oversized = session.send(json!({"kind": "this-message-exceeds-the-limit"}));
    let first = session.send(json!({"kind": "first"}));
    let rate_limited = session.send(json!({"kind": "second"}));
    session.close();
    release_sender.send(()).unwrap();

    assert_eq!(
        invalid_schema.unwrap_err().code(),
        BrokerErrorCode::WebSocketMessageSchemaInvalid
    );
    assert_eq!(
        oversized.unwrap_err().code(),
        BrokerErrorCode::WebSocketMessageTooLarge
    );
    first.unwrap();
    assert_eq!(
        rate_limited.unwrap_err().code(),
        BrokerErrorCode::WebSocketRateLimited
    );
    assert!(matches!(
        session.next_event(),
        Some(WebSocketEvent::Opened { .. })
    ));
    assert!(matches!(session.next_event(), Some(WebSocketEvent::Closed)));
}

#[test]
fn invalid_inbound_schema_is_never_delivered() {
    let transport = Arc::new(ScriptedTransport::new(vec![Ok(VecDeque::from([Ok(
        Some(br#"{"unexpected":"value"}"#.to_vec()),
    )]))]));
    let broker = broker(transport);
    let session = broker
        .guest_api()
        .open(WebSocketOpenRequest::new("ws://127.0.0.1:9000/events"))
        .unwrap();

    assert!(matches!(
        session.next_event(),
        Some(WebSocketEvent::Opened { .. })
    ));
    assert!(matches!(
        session.next_event(),
        Some(WebSocketEvent::Error(error))
            if error.code() == BrokerErrorCode::WebSocketMessageSchemaInvalid
    ));
    assert!(matches!(session.next_event(), Some(WebSocketEvent::Closed)));
}
