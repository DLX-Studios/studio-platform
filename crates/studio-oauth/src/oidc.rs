//! OpenID Connect ID token verification.
//!
//! Some providers assert identity only through a signed ID token and publish no profile
//! endpoint at all. The host verifies that assertion itself rather than trusting it: fetch the
//! provider's published key set over the certificate-validating client, select the single key
//! named by the token header, verify the RS256 signature, then assert issuer, audience, expiry,
//! and the nonce the host generated for this authorization request.
//!
//! Only RS256 is accepted. Allowing the token to name its own algorithm would let an unsigned or
//! symmetrically-signed token be verified against a published RSA key.

use std::{
    sync::{Arc, RwLock},
    time::{Duration, Instant},
};

use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode, decode_header, jwk::JwkSet};
use serde_json::Value;
use studio_net::{
    HttpsClient, OutgoingRequest, ProductionHttpTransport, TransportLimits,
    declaration::HttpMethod, transport::HttpTransport,
};

use crate::{OAuthError, OAuthErrorCode, https_url};

const MAX_JWKS_BYTES: usize = 256 * 1024;
const JWKS_TIMEOUT: Duration = Duration::from_secs(30);
/// Bound on how long a fetched key set is reused. Keys older than this are refetched so a
/// rotated-out signing key cannot stay usable for the life of a long-running host.
const JWKS_TTL: Duration = Duration::from_secs(3600);

/// Host-owned retrieval seam for a provider's published key set.
///
/// Implementations own the transport and must never expose a document to guests.
pub trait JwksProvider: Send + Sync {
    /// Fetch the current key set published at a JWKS document URI.
    fn fetch(&self, uri: &str) -> Result<JwkSet, OAuthError>;
}

/// Key retrieval over the host's certificate-validating HTTPS client.
pub struct HttpsJwksProvider {
    http: ProductionHttpTransport,
}

impl std::fmt::Debug for HttpsJwksProvider {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("HttpsJwksProvider")
    }
}

impl HttpsJwksProvider {
    /// Construct the adapter over a host-provided TLS client.
    #[must_use]
    pub fn new(client: Arc<dyn HttpsClient>) -> Self {
        Self {
            http: ProductionHttpTransport::new(
                client,
                TransportLimits {
                    max_response_bytes: MAX_JWKS_BYTES,
                    ..TransportLimits::default()
                },
            ),
        }
    }
}

impl JwksProvider for HttpsJwksProvider {
    fn fetch(&self, uri: &str) -> Result<JwkSet, OAuthError> {
        if !https_url(uri) {
            return Err(OAuthError::new(OAuthErrorCode::IdTokenInvalid));
        }
        let response = self
            .http
            .execute(OutgoingRequest {
                method: HttpMethod::Get,
                url: uri.to_owned(),
                headers: vec![
                    (String::from("accept"), String::from("application/json")),
                    (String::from("user-agent"), String::from("studio-oauth/1.0")),
                ],
                body: None,
                timeout: JWKS_TIMEOUT,
            })
            .map_err(|_| OAuthError::new(OAuthErrorCode::IdTokenInvalid))?;
        if !(200..300).contains(&response.status) || response.body.len() > MAX_JWKS_BYTES {
            return Err(OAuthError::new(OAuthErrorCode::IdTokenInvalid));
        }
        serde_json::from_slice(&response.body)
            .map_err(|_| OAuthError::new(OAuthErrorCode::IdTokenInvalid))
    }
}

struct CachedKeys {
    fetched: Instant,
    keys: JwkSet,
}

/// Verifies ID tokens against one provider's published key set.
pub struct IdTokenVerifier {
    issuer: String,
    jwks_uri: String,
    provider: Arc<dyn JwksProvider>,
    cached: RwLock<Option<CachedKeys>>,
    ttl: Duration,
}

impl std::fmt::Debug for IdTokenVerifier {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("IdTokenVerifier")
            .field("issuer", &self.issuer)
            .field("jwks_uri", &self.jwks_uri)
            .finish_non_exhaustive()
    }
}

impl IdTokenVerifier {
    /// Construct a verifier bound to one provider issuer and key set.
    pub fn new(issuer: String, jwks_uri: String, provider: Arc<dyn JwksProvider>) -> Self {
        Self {
            issuer,
            jwks_uri,
            provider,
            cached: RwLock::new(None),
            ttl: JWKS_TTL,
        }
    }

    /// Override how long a fetched key set is reused.
    pub fn set_cache_ttl(&mut self, ttl: Duration) {
        self.ttl = ttl;
    }

    /// Verify a signed ID token and return its claims.
    ///
    ///
    /// `client_id` is the audience the token must name. `nonce` is the value the host generated
    /// for this authorization request, and is `None` only when refreshing, where no new
    /// authorization request exists to bind against.
    ///
    /// The audience check is never optional: without it any Apple or Google token would verify
    /// here. Neither is the nonce check on a fresh authorization, because without it a token
    /// captured from another session replays into this one. On refresh the binding comes from
    /// the refresh token, which exists only because the original exchange passed both.
    pub fn verify(
        &self,
        token: &str,
        client_id: &str,
        nonce: Option<&str>,
    ) -> Result<Value, OAuthError> {
        let invalid = OAuthError::new(OAuthErrorCode::IdTokenInvalid);
        if token.is_empty() || client_id.is_empty() || nonce.is_some_and(str::is_empty) {
            return Err(invalid);
        }
        let header = decode_header(token).map_err(|_| invalid)?;
        if header.alg != Algorithm::RS256 {
            return Err(invalid);
        }
        // Refuse to search the whole key set: a token must name the key it was signed with.
        let kid = header.kid.as_deref().ok_or(invalid)?;
        let keys = self.keys_for(kid)?;
        let jwk = keys.find(kid).ok_or(invalid)?;
        let key = DecodingKey::from_jwk(jwk).map_err(|_| invalid)?;

        let mut validation = Validation::new(Algorithm::RS256);
        validation.set_issuer(&[self.issuer.as_str()]);
        validation.set_audience(&[client_id]);
        validation.set_required_spec_claims(&["exp", "iss", "aud", "sub"]);
        // The audience is asserted explicitly above, so a missing one must fail rather than be
        // skipped.
        validation.validate_aud = true;

        let decoded = decode::<Value>(token, &key, &validation).map_err(|_| invalid)?;
        if let Some(nonce) = nonce {
            let echoed = decoded
                .claims
                .get("nonce")
                .and_then(Value::as_str)
                .ok_or(invalid)?;
            if echoed != nonce {
                return Err(invalid);
            }
        }
        Ok(decoded.claims)
    }

    /// Return the key set containing `kid`, refetching when the cache is stale or lacks it.
    ///
    /// Key rotation publishes a new key before retiring the old one, so an unknown `kid` is a
    /// signal to refetch once rather than a reason to reject the authorization outright.
    fn keys_for(&self, kid: &str) -> Result<JwkSet, OAuthError> {
        if let Ok(cache) = self.cached.read()
            && let Some(cached) = cache.as_ref()
            && cached.fetched.elapsed() < self.ttl
            && cached.keys.find(kid).is_some()
        {
            return Ok(cached.keys.clone());
        }
        let keys = self.provider.fetch(&self.jwks_uri)?;
        if keys.find(kid).is_none() {
            return Err(OAuthError::new(OAuthErrorCode::IdTokenInvalid));
        }
        if let Ok(mut cache) = self.cached.write() {
            *cache = Some(CachedKeys {
                fetched: Instant::now(),
                keys: keys.clone(),
            });
        }
        Ok(keys)
    }
}
