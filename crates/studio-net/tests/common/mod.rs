//! Shared deterministic fixtures for studio-net integration tests.
//!
//! Everything here runs without network, vaults, or wall-clock sensitivity: the transport is a
//! scripted fake and the credential backend is an in-memory map.

#![allow(missing_docs)]

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{json, Value};
use studio_net::broker::RestBroker;
use studio_net::declaration::{CredentialSource, HttpMethod, RouteGroupDeclaration};
use studio_net::error::{BrokerError, BrokerErrorCode};
use studio_net::guest::{BrokerRequest, StreamEvent};
use studio_net::limits::{BrokerLimits, DeclaredLimits};
use studio_net::transport::{
    ByteStream, HttpTransport, IncomingResponse, OutgoingRequest, TransportError,
};
use studio_security::{
    ApplicationEnvironment, CredentialBackend, CredentialBackendError, CredentialBytes,
    CredentialLocator, PluginPrincipal, ProtectedSecretKey, ProtectedSecretStore, SecretInput,
    TrustMode,
};

pub const ORIGIN: &str = "https://api.example.test";

/// In-memory stand-in for the operating-system credential facility.
#[derive(Clone, Default)]
pub struct FakeBackend {
    secrets: Arc<Mutex<HashMap<CredentialLocator, Vec<u8>>>>,
}

impl CredentialBackend for FakeBackend {
    fn set_secret(
        &self,
        locator: &CredentialLocator,
        secret: &[u8],
    ) -> Result<(), CredentialBackendError> {
        self.secrets
            .lock()
            .unwrap()
            .insert(locator.clone(), secret.to_vec());
        Ok(())
    }

    fn get_secret(
        &self,
        locator: &CredentialLocator,
    ) -> Result<CredentialBytes, CredentialBackendError> {
        self.secrets
            .lock()
            .unwrap()
            .get(locator)
            .map(|bytes| CredentialBytes::new(bytes.clone()))
            .ok_or(CredentialBackendError::NotFound)
    }

    fn delete_secret(&self, locator: &CredentialLocator) -> Result<(), CredentialBackendError> {
        self.secrets
            .lock()
            .unwrap()
            .remove(locator)
            .map(|_| ())
            .ok_or(CredentialBackendError::NotFound)
    }
}

/// One scripted stream read step.
pub enum ReadStep {
    /// Deliver bytes.
    Bytes(Vec<u8>),
    /// Pause so the test can interleave (for example, cancellation).
    Wait(Duration),
    /// Fail the underlying connection.
    Fail(TransportError),
    /// Clean end of stream.
    End,
}

/// Scripted transport with recorded requests.
#[derive(Default)]
pub struct ScriptedTransport {
    exchanges: Mutex<VecDeque<Result<IncomingResponse, TransportError>>>,
    plans: Mutex<VecDeque<Vec<ReadStep>>>,
    pub requests: Mutex<Vec<OutgoingRequest>>,
}

impl ScriptedTransport {
    pub fn respond(&self, status: u16, media_type: &str, body: &str) {
        self.exchanges.lock().unwrap().push_back(Ok(IncomingResponse {
            status,
            media_type: Some(media_type.to_owned()),
            body: body.as_bytes().to_vec(),
        }));
    }

    pub fn fail_exchange(&self, error: TransportError) {
        self.exchanges.lock().unwrap().push_back(Err(error));
    }

    pub fn plan_stream(&self, steps: Vec<ReadStep>) {
        self.plans.lock().unwrap().push_back(steps);
    }

    pub fn recorded_requests(&self) -> Vec<OutgoingRequest> {
        self.requests.lock().unwrap().clone()
    }
}

impl HttpTransport for ScriptedTransport {
    fn execute(&self, request: OutgoingRequest) -> Result<IncomingResponse, TransportError> {
        self.requests.lock().unwrap().push(request);
        self.exchanges
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or(Err(TransportError::ConnectionFailure))
    }

    fn open_stream(
        &self,
        request: OutgoingRequest,
    ) -> Result<Box<dyn ByteStream + '_>, TransportError> {
        self.requests.lock().unwrap().push(request);
        let steps = self
            .plans
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or_else(|| vec![ReadStep::End]);
        Ok(Box::new(ScriptedStreamReader {
            steps: Mutex::new(VecDeque::from(steps)),
        }))
    }
}

struct ScriptedStreamReader {
    steps: Mutex<VecDeque<ReadStep>>,
}

impl ByteStream for ScriptedStreamReader {
    fn read_chunk(&mut self) -> Result<Option<Vec<u8>>, TransportError> {
        let mut steps = self.steps.lock().unwrap();
        match steps.pop_front() {
            Some(ReadStep::Bytes(bytes)) => Ok(Some(bytes)),
            Some(ReadStep::Wait(duration)) => {
                drop(steps);
                std::thread::sleep(duration);
                Ok(Some(Vec::new()))
            }
            Some(ReadStep::Fail(error)) => Err(error),
            Some(ReadStep::End) | None => Ok(None),
        }
    }
}

/// Broker fixture with its scripted transport.
pub struct Fixture<'store> {
    pub transport: Arc<ScriptedTransport>,
    pub broker: Arc<RestBroker<'store>>,
}

pub fn broker<'store>(groups: &[RouteGroupDeclaration]) -> Fixture<'store> {
    broker_with_limits(groups, BrokerLimits::default())
}

pub fn broker_with_limits<'store>(
    groups: &[RouteGroupDeclaration],
    limits: BrokerLimits,
) -> Fixture<'store> {
    broker_with_limits_impl(groups, limits)
}

fn broker_with_limits_impl<'store>(
    groups: &[RouteGroupDeclaration],
    limits: BrokerLimits,
) -> Fixture<'store> {
    assert!(limits.validate().is_ok(), "fixture limits must be valid");
    let transport = Arc::new(ScriptedTransport::default());
    let mut broker = RestBroker::new(
        Arc::clone(&transport) as Arc<dyn HttpTransport>,
        limits,
    );
    for group in groups {
        broker
            .declare_group(group)
            .expect("fixture declaration must compile");
    }
    Fixture {
        transport,
        broker: Arc::new(broker),
    }
}

pub fn json_api_group() -> RouteGroupDeclaration {
    RouteGroupDeclaration {
        id: String::from("json-api"),
        origins: vec![ORIGIN.to_owned()],
        methods: vec![HttpMethod::Get],
        paths: vec![
            String::from("/v1/items"),
            String::from("/v1/items/{id}"),
            String::from("/v1/search/**"),
        ],
        allowed_headers: vec![String::from("x-request-id")],
        credential: CredentialSource::Public,
        request_schema: None,
        response_schema: Some(json!({
            "type": "object",
            "properties": {"id": {"type": "string"}, "name": {"type": "string"}},
            "required": ["id", "name"],
        })),
        streaming: None,
        limits: DeclaredLimits::default(),
    }
}

/// Body-carrying route group with a declared request schema.
pub fn post_items_group() -> RouteGroupDeclaration {
    RouteGroupDeclaration {
        id: String::from("json-post"),
        origins: vec![ORIGIN.to_owned()],
        methods: vec![HttpMethod::Post],
        paths: vec![String::from("/v1/items")],
        allowed_headers: vec![String::from("x-request-id")],
        credential: CredentialSource::Public,
        request_schema: Some(json!({
            "type": "object",
            "properties": {"name": {"type": "string"}},
            "required": ["name"],
        })),
        response_schema: Some(json!({
            "type": "object",
            "properties": {"id": {"type": "string"}, "name": {"type": "string"}},
            "required": ["id", "name"],
        })),
        streaming: None,
        limits: DeclaredLimits::default(),
    }
}

pub fn sse_group() -> RouteGroupDeclaration {
    RouteGroupDeclaration {
        id: String::from("events"),
        origins: vec![ORIGIN.to_owned()],
        methods: vec![HttpMethod::Get],
        paths: vec![String::from("/v1/stream")],
        allowed_headers: Vec::new(),
        credential: CredentialSource::Public,
        request_schema: None,
        response_schema: None,
        streaming: Some(studio_net::declaration::StreamingDeclaration {
            chunk_schema: json!({
                "type": "object",
                "properties": {"text": {"type": "string"}},
                "required": ["text"],
            }),
            reconnects: Some(1),
            retry_base_delay_ms: Some(50),
        }),
        limits: DeclaredLimits::default(),
    }
}

pub fn get_items_request(path: &str) -> BrokerRequest {
    BrokerRequest::new(ORIGIN, HttpMethod::Get, path)
}

pub fn code_of(error: &BrokerError) -> BrokerErrorCode {
    error.code()
}

/// Configured protected secret backed by [`FakeBackend`].
///
/// Keep the returned fixture alive for the lifetime of the injection handle.
pub struct SecretFixture {
    pub store: ProtectedSecretStore<FakeBackend>,
    pub key: ProtectedSecretKey,
}

impl SecretFixture {
    pub fn injection_handle(
        &self,
        names: &[ProtectedSecretKey],
    ) -> studio_security::BrokerSecretInjectionHandle<'_, FakeBackend> {
        self.store.broker_injection_handle(names.iter().cloned())
    }
}

pub fn secret_fixture(name: &str, value: &str) -> SecretFixture {
    let store = ProtectedSecretStore::new(FakeBackend::default());
    let principal = PluginPrincipal::new_verified(
        "publisher.example",
        "signing-key-1",
        "com.example.app",
        [1_u8; 32],
        [2_u8; 16],
        TrustMode::Production,
    )
    .expect("principal");
    let application = store
        .for_application(&principal, ApplicationEnvironment::Development)
        .expect("partition");
    let key =
        ProtectedSecretKey::new(name, format!("REST credential for route group using {name}"))
            .expect("key");
    application
        .configure(&key, SecretInput::new(value.as_bytes().to_vec()).expect("secret input"))
        .expect("configure");
    SecretFixture { store, key }
}

pub fn named_secret_group(header: &str, prefix: Option<&str>) -> RouteGroupDeclaration {
    RouteGroupDeclaration {
        id: String::from("secret-api"),
        origins: vec![ORIGIN.to_owned()],
        methods: vec![HttpMethod::Get],
        paths: vec![String::from("/v1/private")],
        allowed_headers: Vec::new(),
        credential: CredentialSource::NamedSecret {
            name: String::from("payments.key"),
            header: header.to_owned(),
            prefix: prefix.map(str::to_owned),
        },
        request_schema: None,
        response_schema: None,
        streaming: None,
        limits: DeclaredLimits::default(),
    }
}

pub fn drain_stream(handle: &studio_net::guest::StreamHandle) -> Vec<StreamEvent> {
    let mut events = Vec::new();
    while let Some(event) = handle.next_event() {
        let terminal = matches!(
            event,
            StreamEvent::Completed | StreamEvent::Cancelled | StreamEvent::Failed(_)
        );
        events.push(event);
        if terminal {
            break;
        }
    }
    events
}

pub fn chunk_values(events: &[StreamEvent]) -> Vec<Value> {
    events
        .iter()
        .filter_map(|event| match event {
            StreamEvent::Chunk(value) => Some(value.clone()),
            _ => None,
        })
        .collect()
}
