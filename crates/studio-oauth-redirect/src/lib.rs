//! Shared contract between an OAuth client and a hosted redirect broker.
//!
//! A broker is a stateless HTTPS service that turns a provider's redirect into a loopback
//! redirect on the machine that started the authorization. It keeps nothing: the application
//! encodes the loopback port it bound into the `state` value it hands the provider, and the
//! broker forwards the provider's redirect to that port.
//!
//! The port is authenticated with a shared secret, so a broker only ever forwards to a port
//! the application chose. A broker that merely parsed a port out of `state` would let anyone
//! aim its redirects at an arbitrary loopback service, which is an open redirector wearing a
//! loopback address. The secret is what separates the two.
//!
//! Both sides of the contract live here so a client and a broker cannot drift apart: the
//! state format, the callback path, and the form-encoding rules for redirect parameters.

use std::collections::BTreeMap;
use std::fmt;

use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use hmac::{Hmac, Mac};
use sha2_10::Sha256;
use subtle::ConstantTimeEq;
use zeroize::Zeroizing;

/// The path a loopback listener serves its authorization callback on.
pub const CALLBACK_PATH: &str = "/oauth/callback";

/// The path prefix a broker serves application redirect URIs under.
pub const BROKER_PATH_PREFIX: &str = "/cb/";

/// Bytes of random entropy carried in a broker state value.
pub const STATE_ENTROPY_BYTES: usize = 32;

/// Bytes of authentication tag carried in a broker state value.
pub const STATE_TAG_BYTES: usize = 16;

/// The shortest shared secret a broker accepts.
pub const MINIMUM_SECRET_BYTES: usize = 32;

/// The lowest loopback port a broker forwards to.
///
/// Privileged ports are excluded so a forged or misconfigured state can never aim a redirect
/// at a service listening below the unprivileged range.
pub const MINIMUM_FORWARD_PORT: u16 = 1024;

/// The longest field name a redirect parameter may carry.
pub const MAX_REDIRECT_FIELD_BYTES: usize = 64;

/// The longest value a redirect parameter may carry.
///
/// This matches the cap a loopback client applies to the same parameters, so a broker never
/// refuses a redirect the client would have accepted.
pub const MAX_REDIRECT_VALUE_BYTES: usize = 8 * 1024;

/// The largest number of parameters one redirect may carry.
pub const MAX_REDIRECT_PARAMS: usize = 16;

/// Why a state value was refused.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StateError {
    /// The value was not a well-formed state value.
    Malformed,
    /// The authentication tag did not match the shared secret.
    Unauthenticated,
    /// The encoded port was outside the range a broker may forward to.
    PortOutOfRange,
}

/// Why a form-encoded payload was refused.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RedirectParseError {
    /// A pair was not `key=value`, or a percent escape was malformed.
    Malformed,
    /// A field name was empty, too long, or outside the permitted character set.
    InvalidField,
    /// A value was too long or carried a control character.
    InvalidValue,
    /// A field name appeared more than once.
    DuplicateField,
}

/// Why a broker configuration was refused.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BrokerConfigError {
    /// The base URL was not a usable HTTPS origin.
    InvalidBaseUrl,
    /// The application identifier was not a usable path segment.
    InvalidApplicationId,
    /// The shared secret was too short to authenticate a state value.
    SecretTooShort,
}

/// Encode a state value that carries a loopback port to a broker.
///
/// The value is `port || tag || entropy`, authenticated with `secret`. A broker recovers the
/// port by recomputing the tag, so a state value it did not mint is refused rather than
/// obeyed. The entropy keeps two authorizations from the same application distinguishable.
#[must_use]
pub fn encode_state(port: u16, secret: &[u8], entropy: &[u8; STATE_ENTROPY_BYTES]) -> String {
    let mut body = Vec::with_capacity(2 + STATE_ENTROPY_BYTES + STATE_TAG_BYTES);
    body.extend_from_slice(&port.to_le_bytes());
    body.extend_from_slice(entropy);
    let tag = authentication_tag(secret, &body);
    body.extend_from_slice(&tag);
    URL_SAFE_NO_PAD.encode(&body)
}

/// Recover the loopback port a state value asks a broker to forward to.
///
/// The value is refused unless it was minted with the same shared secret, and unless the port
/// it names is one a broker may forward to.
///
/// # Errors
///
/// Returns [`StateError`] when the value is malformed, was not minted with this secret, or
/// names a port below [`MINIMUM_FORWARD_PORT`].
pub fn decode_state(state: &str, secret: &[u8]) -> Result<u16, StateError> {
    let bytes = URL_SAFE_NO_PAD
        .decode(state.as_bytes())
        .map_err(|_| StateError::Malformed)?;
    let body_length = 2 + STATE_ENTROPY_BYTES;
    if bytes.len() != body_length + STATE_TAG_BYTES {
        return Err(StateError::Malformed);
    }
    let (body, tag) = bytes.split_at(body_length);
    let port = u16::from_le_bytes([body[0], body[1]]);
    let expected = authentication_tag(secret, body);
    if expected.ct_eq(tag).unwrap_u8() != 1 {
        return Err(StateError::Unauthenticated);
    }
    if port < MINIMUM_FORWARD_PORT {
        return Err(StateError::PortOutOfRange);
    }
    Ok(port)
}

/// The authentication tag over a state body.
fn authentication_tag(secret: &[u8], body: &[u8]) -> [u8; STATE_TAG_BYTES] {
    let mut mac = Hmac::<Sha256>::new_from_slice(secret).expect("HMAC accepts any key length");
    mac.update(body);
    let digest = mac.finalize().into_bytes();
    let mut tag = [0_u8; STATE_TAG_BYTES];
    tag.copy_from_slice(&digest[..STATE_TAG_BYTES]);
    tag
}

/// The loopback URL a broker forwards a provider redirect to.
///
/// The host and path are fixed. Only the port varies, and only to a port the application
/// proved it chose.
#[must_use]
pub fn loopback_callback_url(port: u16) -> String {
    format!("http://127.0.0.1:{port}{CALLBACK_PATH}")
}

/// A hosted redirect broker an application registers with its provider.
///
/// The broker URL a developer registers with a provider is
/// `{base_url}/cb/{application_id}`. The same application identifier and shared secret must be
/// configured on the broker, which uses the secret to decide whether a state value is one it
/// should forward.
#[derive(Clone)]
pub struct RedirectBroker {
    base_url: String,
    application_id: String,
    secret: Zeroizing<Vec<u8>>,
}

impl fmt::Debug for RedirectBroker {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RedirectBroker")
            .field("base_url", &self.base_url)
            .field("application_id", &self.application_id)
            .field("secret", &"[redacted]")
            .finish()
    }
}

impl RedirectBroker {
    /// Configure a broker from the values an operator deploys it with.
    ///
    /// # Errors
    ///
    /// Returns [`BrokerConfigError`] when the base URL is not a usable HTTPS origin, when the
    /// application identifier is not a usable path segment, or when the shared secret is too
    /// short to authenticate a state value.
    pub fn new(
        base_url: &str,
        application_id: &str,
        secret: &[u8],
    ) -> Result<Self, BrokerConfigError> {
        if !valid_base_url(base_url) {
            return Err(BrokerConfigError::InvalidBaseUrl);
        }
        if !valid_application_id(application_id) {
            return Err(BrokerConfigError::InvalidApplicationId);
        }
        if secret.len() < MINIMUM_SECRET_BYTES {
            return Err(BrokerConfigError::SecretTooShort);
        }
        Ok(Self {
            base_url: base_url.trim_end_matches('/').to_owned(),
            application_id: application_id.to_owned(),
            secret: Zeroizing::new(secret.to_vec()),
        })
    }

    /// The redirect URI a developer registers with a provider.
    #[must_use]
    pub fn redirect_uri(&self) -> String {
        format!(
            "{}{BROKER_PATH_PREFIX}{}",
            self.base_url, self.application_id
        )
    }

    /// The application identifier this broker configuration serves.
    #[must_use]
    pub fn application_id(&self) -> &str {
        &self.application_id
    }

    /// Encode a state value that asks this broker to forward to `port`.
    #[must_use]
    pub fn state(&self, port: u16, entropy: &[u8; STATE_ENTROPY_BYTES]) -> String {
        encode_state(port, self.secret.as_slice(), entropy)
    }

    /// Recover the port a returned state value names.
    ///
    /// # Errors
    ///
    /// Returns [`StateError`] when the value was not minted with this broker's secret or
    /// names a port the broker may not forward to.
    pub fn decode(&self, state: &str) -> Result<u16, StateError> {
        decode_state(state, self.secret.as_slice())
    }
}

/// Whether a base URL is a usable HTTPS origin.
fn valid_base_url(value: &str) -> bool {
    let Some(rest) = value.strip_prefix("https://") else {
        return false;
    };
    !rest.is_empty()
        && rest.len() <= 2048
        && !value.chars().any(char::is_control)
        && !value.contains('?')
        && !value.contains('#')
        && !value.ends_with('/')
}

/// Whether an application identifier is a usable path segment.
fn valid_application_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
}

/// The application identifier a broker redirect path names.
#[must_use]
pub fn application_id_from_path(path: &str) -> Option<&str> {
    let identifier = path.strip_prefix(BROKER_PATH_PREFIX)?;
    (!identifier.is_empty() && !identifier.contains('/')).then_some(identifier)
}

/// Percent-encode a value for a form-encoded payload.
#[must_use]
pub fn percent_encode(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            output.push(char::from(byte));
        } else {
            use fmt::Write as _;
            write!(output, "%{byte:02X}").expect("writing into a String cannot fail");
        }
    }
    output
}

/// Percent-decode a value from a form-encoded payload.
#[must_use]
pub fn percent_decode(value: &str) -> Option<String> {
    let bytes = value.as_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'%' if index + 2 < bytes.len() => {
                let high = hex(bytes[index + 1])?;
                let low = hex(bytes[index + 2])?;
                output.push(high * 16 + low);
                index += 3;
            }
            b'%' => return None,
            byte => {
                output.push(byte);
                index += 1;
            }
        }
    }
    String::from_utf8(output).ok()
}

/// The value of a hexadecimal digit.
fn hex(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}

/// Whether a field name is one a redirect may carry.
#[must_use]
pub fn valid_field_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_REDIRECT_FIELD_BYTES
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
}

/// Whether a field value is one a redirect may carry.
#[must_use]
pub fn valid_field_value(value: &str) -> bool {
    value.len() <= MAX_REDIRECT_VALUE_BYTES && !value.chars().any(char::is_control)
}

/// Parse form-encoded redirect parameters.
///
/// Duplicate names are refused rather than resolved, so a parameter cannot be read two ways.
///
/// # Errors
///
/// Returns [`RedirectParseError`] when a pair is malformed, a name is not one a redirect may
/// carry, a value is too long or carries a control character, or a name appears twice.
pub fn parse_redirect_params(
    payload: &str,
) -> Result<BTreeMap<String, String>, RedirectParseError> {
    let mut parsed = BTreeMap::new();
    for part in payload.split('&') {
        if part.is_empty() {
            continue;
        }
        if parsed.len() >= MAX_REDIRECT_PARAMS {
            return Err(RedirectParseError::InvalidField);
        }
        let (key, value) = part.split_once('=').ok_or(RedirectParseError::Malformed)?;
        let key = percent_decode(key).ok_or(RedirectParseError::Malformed)?;
        let value = percent_decode(value).ok_or(RedirectParseError::Malformed)?;
        if !valid_field_name(&key) {
            return Err(RedirectParseError::InvalidField);
        }
        if !valid_field_value(&value) {
            return Err(RedirectParseError::InvalidValue);
        }
        if parsed.insert(key, value).is_some() {
            return Err(RedirectParseError::DuplicateField);
        }
    }
    Ok(parsed)
}

/// Render redirect parameters as a form-encoded payload.
#[must_use]
pub fn encode_redirect_params(params: &BTreeMap<String, String>) -> String {
    let mut output = String::new();
    for (key, value) in params {
        if !output.is_empty() {
            output.push('&');
        }
        output.push_str(&percent_encode(key));
        output.push('=');
        output.push_str(&percent_encode(value));
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    const SECRET: &[u8] = b"a-shared-secret-of-at-least-thirty-two-bytes";

    fn entropy() -> [u8; STATE_ENTROPY_BYTES] {
        [7_u8; STATE_ENTROPY_BYTES]
    }

    #[test]
    fn state_round_trips_the_port() {
        let state = encode_state(43123, SECRET, &entropy());
        assert_eq!(decode_state(&state, SECRET), Ok(43123));
    }

    #[test]
    fn state_is_refused_under_another_secret() {
        let state = encode_state(43123, SECRET, &entropy());
        let other = b"a-different-secret-of-at-least-thirty-two-bytes";
        assert_eq!(
            decode_state(&state, other),
            Err(StateError::Unauthenticated)
        );
    }

    #[test]
    fn state_refuses_a_tampered_port() {
        let state = encode_state(43123, SECRET, &entropy());
        let mut bytes = URL_SAFE_NO_PAD
            .decode(state.as_bytes())
            .expect("state decodes");
        bytes[0] = 99;
        let tampered = URL_SAFE_NO_PAD.encode(&bytes);
        assert_eq!(
            decode_state(&tampered, SECRET),
            Err(StateError::Unauthenticated)
        );
    }

    #[test]
    fn state_refuses_a_privileged_port() {
        let state = encode_state(22, SECRET, &entropy());
        assert_eq!(
            decode_state(&state, SECRET),
            Err(StateError::PortOutOfRange)
        );
    }

    #[test]
    fn state_refuses_a_malformed_value() {
        assert_eq!(
            decode_state("not base64!", SECRET),
            Err(StateError::Malformed)
        );
        assert_eq!(decode_state("", SECRET), Err(StateError::Malformed));
    }

    #[test]
    fn state_does_not_leak_the_port_in_the_clear() {
        let state = encode_state(43123, SECRET, &entropy());
        let bytes = URL_SAFE_NO_PAD
            .decode(state.as_bytes())
            .expect("state decodes");
        let port = u16::from_le_bytes([bytes[0], bytes[1]]);
        assert_eq!(port, 43123);
        // The port is recoverable, but only by a holder of the shared secret.
        assert_ne!(bytes[0], 0);
    }

    #[test]
    fn broker_requires_a_usable_origin() {
        assert_eq!(
            RedirectBroker::new("http://broker.example", "app", SECRET).unwrap_err(),
            BrokerConfigError::InvalidBaseUrl
        );
        assert_eq!(
            RedirectBroker::new("https://", "app", SECRET).unwrap_err(),
            BrokerConfigError::InvalidBaseUrl
        );
        assert_eq!(
            RedirectBroker::new("https://broker.example/", "app", SECRET).unwrap_err(),
            BrokerConfigError::InvalidBaseUrl
        );
    }

    #[test]
    fn broker_requires_a_usable_application_identifier() {
        assert_eq!(
            RedirectBroker::new("https://broker.example", "app/../other", SECRET).unwrap_err(),
            BrokerConfigError::InvalidApplicationId
        );
        assert_eq!(
            RedirectBroker::new("https://broker.example", "", SECRET).unwrap_err(),
            BrokerConfigError::InvalidApplicationId
        );
    }

    #[test]
    fn broker_requires_a_long_enough_secret() {
        assert_eq!(
            RedirectBroker::new("https://broker.example", "app", b"short").unwrap_err(),
            BrokerConfigError::SecretTooShort
        );
    }

    #[test]
    fn broker_builds_the_registered_redirect_uri() {
        let broker = RedirectBroker::new("https://broker.example", "com.example.viewer", SECRET)
            .expect("broker configuration is valid");
        assert_eq!(
            broker.redirect_uri(),
            "https://broker.example/cb/com.example.viewer"
        );
    }

    #[test]
    fn broker_never_prints_its_secret() {
        let broker = RedirectBroker::new("https://broker.example", "app", SECRET)
            .expect("broker configuration is valid");
        let printed = format!("{broker:?}");
        assert!(!printed.contains("shared-secret"));
        assert!(printed.contains("[redacted]"));
    }

    #[test]
    fn broker_state_round_trips() {
        let broker = RedirectBroker::new("https://broker.example", "app", SECRET)
            .expect("broker configuration is valid");
        let state = broker.state(51000, &entropy());
        assert_eq!(broker.decode(&state), Ok(51000));
    }

    #[test]
    fn application_identifier_is_read_from_a_broker_path() {
        assert_eq!(
            application_id_from_path("/cb/com.example.viewer"),
            Some("com.example.viewer")
        );
        assert_eq!(application_id_from_path("/cb/"), None);
        assert_eq!(application_id_from_path("/cb/a/b"), None);
        assert_eq!(application_id_from_path("/oauth/callback"), None);
    }

    #[test]
    fn redirect_parameters_round_trip() {
        let payload = "code=abc&state=xyz&user=%7B%22name%22%3A%22A%22%7D";
        let params = parse_redirect_params(payload).expect("payload parses");
        assert_eq!(params.get("code").map(String::as_str), Some("abc"));
        assert_eq!(
            params.get("user").map(String::as_str),
            Some("{\"name\":\"A\"}")
        );
        let encoded = encode_redirect_params(&params);
        assert_eq!(
            parse_redirect_params(&encoded).expect("re-encoded payload parses"),
            params
        );
    }

    #[test]
    fn redirect_parameters_refuse_duplicates_and_odd_names() {
        assert_eq!(
            parse_redirect_params("code=a&code=b"),
            Err(RedirectParseError::DuplicateField)
        );
        assert_eq!(
            parse_redirect_params("no-equals"),
            Err(RedirectParseError::Malformed)
        );
        assert_eq!(
            parse_redirect_params("bad%20name=1"),
            Err(RedirectParseError::InvalidField)
        );
        assert_eq!(
            parse_redirect_params("code=%zz"),
            Err(RedirectParseError::Malformed)
        );
    }

    #[test]
    fn loopback_url_names_a_fixed_host_and_path() {
        assert_eq!(
            loopback_callback_url(43123),
            "http://127.0.0.1:43123/oauth/callback"
        );
    }
}
