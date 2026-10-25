//! The broker service binary.
//!
//! The binary reads its configuration from the environment, binds the listener, prints the
//! listen address once, and hands the listener to [`studio_oauth_broker::serve`]. TLS is
//! terminated by the reverse proxy in front of the service, so the service itself never performs
//! a handshake.

use std::io::{self, Write};
use std::net::TcpListener;
use std::process::ExitCode;

use studio_oauth_broker::{BrokerConfig, BrokerConfigError, serve};
use studio_oauth_redirect::MINIMUM_SECRET_BYTES;
use zeroize::Zeroizing;

/// The environment variable carrying the shared secret.
const SECRET_VARIABLE: &str = "STUDIO_OAUTH_BROKER_SECRET";

/// The environment variable carrying the listen address.
const LISTEN_VARIABLE: &str = "STUDIO_OAUTH_BROKER_LISTEN";

/// The environment variable carrying the application allowlist.
const APPLICATIONS_VARIABLE: &str = "STUDIO_OAUTH_BROKER_APPLICATIONS";

/// The listen address used when none is configured.
const DEFAULT_LISTEN: &str = "127.0.0.1:8080";

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("studio-oauth-broker: {error}");
            ExitCode::FAILURE
        }
    }
}

/// Configure the broker from the environment and serve until the listener fails.
///
/// # Errors
///
/// Returns a message when the environment is unusable or the listener cannot be bound.
fn run() -> Result<(), String> {
    let secret = read_secret()?;
    let applications = read_applications();
    let identifiers: Vec<&str> = applications.iter().map(String::as_str).collect();
    let config = BrokerConfig::new(secret.as_bytes(), &identifiers).map_err(config_error)?;

    let address = std::env::var(LISTEN_VARIABLE).unwrap_or_else(|_| String::from(DEFAULT_LISTEN));
    let listener = TcpListener::bind(&address)
        .map_err(|error| format!("cannot listen on {address}: {error}"))?;
    let local = listener
        .local_addr()
        .map_err(|error| format!("cannot read the listen address: {error}"))?;

    println!("studio-oauth-broker listening on {local}");
    if config.is_unrestricted() {
        println!("studio-oauth-broker serving every application");
    } else {
        println!(
            "studio-oauth-broker serving {} application(s)",
            config.application_count()
        );
    }
    io::stdout()
        .flush()
        .map_err(|error| format!("cannot write the startup line: {error}"))?;

    serve(listener, &config).map_err(|error| format!("the listener failed: {error}"))
}

/// Read the shared secret from the environment.
///
/// # Errors
///
/// Returns a message when the variable is absent, is not text, or is shorter than the shared
/// minimum a state value is authenticated with.
fn read_secret() -> Result<Zeroizing<String>, String> {
    match std::env::var(SECRET_VARIABLE) {
        Ok(value) => {
            let value = Zeroizing::new(value);
            if value.len() < MINIMUM_SECRET_BYTES {
                return Err(format!(
                    "{SECRET_VARIABLE} must be at least {MINIMUM_SECRET_BYTES} bytes, got {}",
                    value.len()
                ));
            }
            Ok(value)
        }
        Err(std::env::VarError::NotPresent) => Err(format!("{SECRET_VARIABLE} is required")),
        Err(std::env::VarError::NotUnicode(_)) => {
            Err(format!("{SECRET_VARIABLE} must be valid UTF-8"))
        }
    }
}

/// Read the application allowlist from the environment.
///
/// An absent variable means every syntactically valid application is served.
fn read_applications() -> Vec<String> {
    let Ok(raw) = std::env::var(APPLICATIONS_VARIABLE) else {
        return Vec::new();
    };
    raw.split(',')
        .map(str::trim)
        .filter(|identifier| !identifier.is_empty())
        .map(String::from)
        .collect()
}

/// The message for a refused configuration.
fn config_error(error: BrokerConfigError) -> String {
    match error {
        BrokerConfigError::SecretTooShort => {
            format!("{SECRET_VARIABLE} must be at least {MINIMUM_SECRET_BYTES} bytes")
        }
    }
}
