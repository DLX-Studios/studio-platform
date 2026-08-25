//! Per-route-group credential resolution.
//!
//! Three declared sources exist: `public` sends nothing, `oauthProviderSession` resolves through
//! the typed seam below (filled by the OAuth provider-plugin milestone), and `namedSecret`
//! injects a protected value strictly at send time through
//! [`studio_security::BrokerSecretInjector`]. Injected values flow from the credential facility
//! into the outgoing header inside one function call; they are never cloned into guest memory,
//! never formatted, and are registered with the redaction scrubber so no later diagnostic can
//! echo them.

use std::sync::{Arc, Mutex};

use studio_security::{
    BrokerCredentialError, BrokerCredentialSink, ProtectedSecretErrorCode, ProtectedSecretKey,
    ProtectedSecretError, SensitiveValueFilter,
};

use crate::declaration::CompiledRouteGroup;
use crate::error::{BrokerError, BrokerErrorCode};

/// Object-safe adapter around the sealed [`studio_security::BrokerSecretInjector`] capability so
/// the broker can hold one injector behind an `Arc<dyn ...>` without carrying credential-backend
/// generics.
pub trait NamedSecretInjector: Send + Sync {
    /// Forward to the underlying send-time injection hook.
    ///
    /// # Errors
    ///
    /// Returns the underlying safe [`ProtectedSecretError`].
    fn inject_named_secret(
        &self,
        key: &ProtectedSecretKey,
        sink: &mut dyn BrokerCredentialSink,
    ) -> Result<(), ProtectedSecretError>;
}

impl<B: studio_security::CredentialBackend> NamedSecretInjector
    for studio_security::BrokerSecretInjectionHandle<'_, B>
{
    fn inject_named_secret(
        &self,
        key: &ProtectedSecretKey,
        sink: &mut dyn BrokerCredentialSink,
    ) -> Result<(), ProtectedSecretError> {
        studio_security::BrokerSecretInjector::inject_at_send_time(self, key, sink)
    }
}

/// Typed host seam for OAuth provider-plugin sessions (ticket 21).
///
/// Implementations resolve an active provider session and attach its credential to the bounded
/// request via [`BrokerCredentialSink`], exactly like named-secret injection. Until a resolver is
/// wired, requests to provider-session groups fail closed with
/// [`BrokerErrorCode::OauthSessionUnavailable`] instead of falling back to generic network
/// behavior.
pub trait OAuthSessionResolver: Send + Sync {
    /// Attach the active session credential for one provider to the outgoing request.
    ///
    /// # Errors
    ///
    /// Returns safe broker codes only; implementations must not surface provider context.
    fn inject_session(
        &self,
        provider: &str,
        route_group_id: &str,
        sink: &mut dyn BrokerCredentialSink,
    ) -> Result<(), BrokerError>;
}

/// Send-time injection sink appending borrowed secret bytes into the outgoing header set while
/// registering them with the shared redaction scrubber.
pub(crate) struct HeaderInjectionSink<'sink> {
    headers: &'sink mut Vec<(String, String)>,
    header_name: String,
    prefix: Option<String>,
    filter: &'sink Mutex<SensitiveValueFilter>,
}

impl<'sink> HeaderInjectionSink<'sink> {
    pub(crate) fn new(
        headers: &'sink mut Vec<(String, String)>,
        header_name: String,
        prefix: Option<String>,
        filter: &'sink Mutex<SensitiveValueFilter>,
    ) -> Self {
        Self {
            headers,
            header_name,
            prefix,
            filter,
        }
    }
}

impl BrokerCredentialSink for HeaderInjectionSink<'_> {
    fn inject(&mut self, secret: &[u8]) -> Result<(), BrokerCredentialError> {
        let value = std::str::from_utf8(secret).map_err(|_| BrokerCredentialError)?;
        if let Ok(filter) = self.filter.lock() {
            let _ = filter.register_secret(secret);
        }
        let composed = match &self.prefix {
            Some(prefix) => format!("{prefix}{value}"),
            None => value.to_owned(),
        };
        self.headers.push((self.header_name.clone(), composed));
        Ok(())
    }
}

/// Resolve and attach credentials for one admitted group at send time.
///
/// # Errors
///
/// Returns [`BrokerErrorCode::CredentialUnavailable`],
/// [`BrokerErrorCode::InjectionRejected`], or [`BrokerErrorCode::OauthSessionUnavailable`] as
/// stable codes without any credential context.
pub fn resolve(
    group: &CompiledRouteGroup,
    injector: Option<&Arc<dyn NamedSecretInjector + '_>>,
    oauth: Option<&Arc<dyn OAuthSessionResolver>>,
    headers: &mut Vec<(String, String)>,
    filter: &Mutex<SensitiveValueFilter>,
) -> Result<(), BrokerError> {
    match group.credential_kind() {
        "public" => Ok(()),
        "oauth-provider-session" => {
            let Some(resolver) = oauth else {
                return Err(BrokerError::new(BrokerErrorCode::OauthSessionUnavailable));
            };
            let mut sink =
                HeaderInjectionSink::new(headers, String::from("authorization"), None, filter);
            resolver.inject_session(
                group.oauth_provider().unwrap_or_default(),
                group.id(),
                &mut sink,
            )
        }
        "named-secret" => {
            let Some(injector) = injector else {
                return Err(BrokerError::new(BrokerErrorCode::CredentialUnavailable));
            };
            let key = group
                .named_secret_key()
                .expect("named-secret groups carry a key");
            let mut sink = HeaderInjectionSink::new(
                headers,
                group
                    .named_secret_header()
                    .expect("named-secret groups carry a header")
                    .to_owned(),
                group.named_secret_prefix().map(str::to_owned),
                filter,
            );
            injector
                .inject_named_secret(key, &mut sink)
                .map_err(|error| map_injection_error(error.code()))
        }
        _ => Err(BrokerError::new(BrokerErrorCode::DeclarationInvalid)),
    }
}

fn map_injection_error(code: ProtectedSecretErrorCode) -> BrokerError {
    match code {
        ProtectedSecretErrorCode::InjectionRejected => {
            BrokerError::new(BrokerErrorCode::InjectionRejected)
        }
        ProtectedSecretErrorCode::RequestInvalid | ProtectedSecretErrorCode::CredentialRejected => {
            BrokerError::with_detail(
                BrokerErrorCode::DeclarationInvalid,
                String::from("credential reference rejected"),
            )
        }
        ProtectedSecretErrorCode::SecretUnavailable
        | ProtectedSecretErrorCode::BackendUnavailable => {
            BrokerError::new(BrokerErrorCode::CredentialUnavailable)
        }
    }
}
