//! End-to-end checks of the broker service over a real socket.
//!
//! The unit tests in the library cover the decision rules; these tests cover the plumbing around
//! them: the request the service reads, the response it writes, and the status it logs.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;

use studio_oauth_broker::{BrokerConfig, serve};
use studio_oauth_redirect::{STATE_ENTROPY_BYTES, encode_state};

const SECRET: &[u8] = b"a-shared-secret-of-at-least-thirty-two-bytes";
const OTHER_SECRET: &[u8] = b"another-secret-of-at-least-thirty-two-bytes";
const PORT: u16 = 43_123;

/// Start the service on an ephemeral port and return its address.
fn start() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("the listener binds");
    let address = listener
        .local_addr()
        .expect("the listener has an address")
        .to_string();
    let config = BrokerConfig::new(SECRET, &[]).expect("the secret is long enough");
    thread::spawn(move || {
        let _ = serve(listener, &config);
    });
    address
}

/// Send one raw request and read the whole response.
fn request(address: &str, raw: &str) -> String {
    let mut stream = TcpStream::connect(address).expect("the service accepts");
    stream
        .write_all(raw.as_bytes())
        .expect("the request is sent");
    stream.flush().expect("the request is flushed");
    let mut response = String::new();
    stream
        .read_to_string(&mut response)
        .expect("the response is read");
    response
}

/// A state value the service accepts.
fn state() -> String {
    encode_state(PORT, SECRET, &[3_u8; STATE_ENTROPY_BYTES])
}

/// A state value the service must refuse.
fn forged_state() -> String {
    encode_state(PORT, OTHER_SECRET, &[3_u8; STATE_ENTROPY_BYTES])
}

#[test]
fn a_valid_get_is_forwarded_to_the_loopback_port() {
    let address = start();
    let response = request(
        &address,
        &format!(
            "GET /cb/desktop?code=abc123&state={} HTTP/1.1\r\nHost: broker.test\r\n\r\n",
            state()
        ),
    );
    assert!(
        response.starts_with("HTTP/1.1 302 Found"),
        "unexpected response {response}"
    );
    assert!(
        response.contains(&format!(
            "Location: http://127.0.0.1:{PORT}/oauth/callback?"
        )),
        "missing location in {response}"
    );
    assert!(
        response.contains("code=abc123"),
        "missing code in {response}"
    );
    assert!(
        response.contains("Connection: close"),
        "missing close in {response}"
    );
}

#[test]
fn a_form_post_is_forwarded_with_every_provider_field() {
    let address = start();
    let body = format!(
        "code=abc123&state={}&user=%7B%22name%22%3A%22Ada%22%7D",
        state()
    );
    let response = request(
        &address,
        &format!(
            "POST /cb/desktop HTTP/1.1\r\nHost: broker.test\r\nContent-Type: application/x-www-form-urlencoded\r\nContent-Length: {}\r\n\r\n{}",
            body.len(),
            body
        ),
    );
    assert!(
        response.starts_with("HTTP/1.1 302 Found"),
        "unexpected response {response}"
    );
    assert!(
        response.contains("code=abc123"),
        "missing code in {response}"
    );
    assert!(
        response.contains("user=%7B%22name%22%3A%22Ada%22%7D"),
        "missing provider payload in {response}"
    );
}

#[test]
fn a_forged_state_is_refused() {
    let address = start();
    let response = request(
        &address,
        &format!(
            "GET /cb/desktop?code=abc123&state={} HTTP/1.1\r\nHost: broker.test\r\n\r\n",
            forged_state()
        ),
    );
    assert!(
        response.starts_with("HTTP/1.1 400 Bad Request"),
        "unexpected response {response}"
    );
    assert!(
        !response.contains("Location:"),
        "a forged state was forwarded"
    );
    assert!(!response.contains("abc123"), "the code was echoed back");
    assert_eq!(
        response.matches("request refused").count(),
        1,
        "the refusal body is written once in {response}"
    );
}

#[test]
fn an_unknown_path_is_refused() {
    let address = start();
    let response = request(
        &address,
        "GET /oauth/callback?code=abc123 HTTP/1.1\r\nHost: broker.test\r\n\r\n",
    );
    assert!(
        response.starts_with("HTTP/1.1 404 Not Found"),
        "unexpected response {response}"
    );
}

#[test]
fn an_unsupported_method_is_refused() {
    let address = start();
    let response = request(
        &address,
        &format!(
            "DELETE /cb/desktop?code=abc123&state={} HTTP/1.1\r\nHost: broker.test\r\n\r\n",
            state()
        ),
    );
    assert!(
        response.starts_with("HTTP/1.1 405 Method Not Allowed"),
        "unexpected response {response}"
    );
}

#[test]
fn an_oversized_body_is_refused_before_it_is_read() {
    let address = start();
    let response = request(
        &address,
        &format!(
            "POST /cb/desktop HTTP/1.1\r\nHost: broker.test\r\nContent-Type: application/x-www-form-urlencoded\r\nContent-Length: {}\r\n\r\n",
            20_000
        ),
    );
    assert!(
        response.starts_with("HTTP/1.1 413 Payload Too Large"),
        "unexpected response {response}"
    );
}

#[test]
fn a_post_with_the_wrong_content_type_is_refused() {
    let address = start();
    let body = format!("code=abc123&state={}", state());
    let response = request(
        &address,
        &format!(
            "POST /cb/desktop HTTP/1.1\r\nHost: broker.test\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
            body.len(),
            body
        ),
    );
    assert!(
        response.starts_with("HTTP/1.1 400 Bad Request"),
        "unexpected response {response}"
    );
}
