#![allow(missing_docs)]

//! Deterministic source tests for the host-only center wire seam.

use std::sync::Arc;

use serde_json::json;
use studio_host::{
    CenterHttpRequest, CenterHttpResponse, CenterHttpTransport, CenterId,
    CenterProtocolLimits, CenterProtocolServer, CenterServer, CenterStationClient, CenterTopology,
    CenterTransportError, StationSettings, StationWriteResult,
};

struct Loopback {
    server: CenterProtocolServer,
}

impl CenterHttpTransport for Loopback {
    fn request(&self, request: CenterHttpRequest) -> Result<CenterHttpResponse, CenterTransportError> {
        Ok(self.server.handle_http(request))
    }
}

fn settings(name: &str) -> StationSettings {
    StationSettings::new(
        name,
        CenterTopology::SelfHosted {
            endpoint: String::from("hub.local:4317"),
        },
    )
    .expect("settings")
}

fn transport() -> (Arc<Loopback>, CenterServer) {
    let center = CenterServer::new(
        CenterId::new("restaurant").expect("center id"),
        CenterTopology::SelfHosted {
            endpoint: String::from("hub.local:4317"),
        },
    )
    .expect("center");
    let server = CenterProtocolServer::new(center.clone(), CenterProtocolLimits::default())
        .expect("protocol server");
    (Arc::new(Loopback { server }), center)
}

#[test]
fn authenticated_http_clients_converge_and_preserve_stale_conflicts() {
    let (transport, center) = transport();
    let mut station_a = CenterStationClient::enroll(
        Arc::clone(&transport),
        "https://hub.example",
        center.issue_pairing_token().expect("token"),
        settings("counter-a"),
        CenterProtocolLimits::default(),
    )
    .expect("station a");
    let mut station_b = CenterStationClient::enroll(
        Arc::clone(&transport),
        "https://hub.example",
        center.issue_pairing_token().expect("token"),
        settings("counter-b"),
        CenterProtocolLimits::default(),
    )
    .expect("station b");

    station_a
        .set("checks", "check-1", json!({"total": 1200}))
        .expect("authoritative write");
    station_b.sync().expect("pull");
    station_b.disconnect();
    assert!(matches!(
        station_b
            .set("checks", "check-1", json!({"total": 1300}))
            .expect("queued"),
        StationWriteResult::Queued(_)
    ));
    station_a
        .set("checks", "check-1", json!({"total": 1400}))
        .expect("second authoritative write");

    let replay = station_b.reconnect().expect("replay");
    assert!(matches!(replay.as_slice(), [StationWriteResult::Conflict { .. }]));
    assert_eq!(station_b.snapshot().expect("snapshot").revision(), 3);
}

#[test]
fn malformed_or_unauthenticated_wire_requests_fail_closed() {
    let (transport, center) = transport();
    let response = transport
        .request(CenterHttpRequest {
            endpoint: String::from("https://hub.example"),
            path: "/v1/snapshot".to_owned(),
            method: studio_host::CenterHttpMethod::Get,
            headers: Vec::new(),
            body: Vec::new(),
        })
        .expect("response");
    assert_eq!(response.status, 426);
    assert!(center.snapshot().is_ok());
}
