//! Shared provisioning of host services from explicit operator configuration.
#[cfg(feature = "services")]
use crate::host::RedirectBrokerConfig;
use crate::host::{HostConfig, NativeDisplayAvailability, StudioHost};
use std::sync::Arc;
use studio_package::TrustStore;
#[cfg(feature = "services")]
use studio_security::{ProtectedSecretKey, SecretInput};

/// Read the hosted redirect broker configuration from the environment.
///
/// A half-configured broker is a configuration error rather than a fallback: the host would
/// sign in one way while the developer registered the redirect URI another.
#[cfg(feature = "services")]
fn redirect_broker_from_environment() -> Result<Option<RedirectBrokerConfig>, String> {
    match (
        std::env::var("STUDIO_OAUTH_BROKER_URL").ok(),
        std::env::var("STUDIO_OAUTH_BROKER_SECRET").ok(),
    ) {
        (None, None) => Ok(None),
        (Some(url), Some(secret)) => Ok(Some(RedirectBrokerConfig::new(url, secret.into_bytes()))),
        _ => Err(String::from(
            "the OAuth redirect broker needs both STUDIO_OAUTH_BROKER_URL and \
             STUDIO_OAUTH_BROKER_SECRET",
        )),
    }
}

/// Provision a host with the same provider configuration in production and development.
///
/// # Errors
/// Rejects incomplete broker settings and invalid declared secrets.
pub fn host_from_environment(
    trust_store: TrustStore,
    https_client: Arc<dyn studio_net::HttpsClient>,
) -> Result<StudioHost, String> {
    let host = StudioHost::new(
        HostConfig::new(trust_store),
        NativeDisplayAvailability::from_environment(),
    )
    .with_https_client(https_client);
    #[cfg(feature = "services")]
    let host = {
        let mut host = host;
        if let Some(broker) = redirect_broker_from_environment()? {
            host = host.with_redirect_broker(broker);
        }
        for descriptor in studio_package::maintained_oauth_descriptors() {
            let Some(secret_name) = descriptor.required_secret_names.first() else {
                continue;
            };
            let Some(purpose) = descriptor.required_secret_purposes.get(secret_name) else {
                continue;
            };
            let variable = format!("STUDIO_{}_CLIENT_SECRET", descriptor.id.to_uppercase());
            let Ok(secret) = std::env::var(variable) else {
                continue;
            };
            let key =
                ProtectedSecretKey::new(secret_name.clone(), purpose.clone()).map_err(|_| {
                    format!(
                        "protected client secret declaration is invalid: {}",
                        descriptor.id
                    )
                })?;
            let value = SecretInput::new(secret.into_bytes())
                .map_err(|_| format!("protected client secret is invalid: {}", descriptor.id))?;
            host = host.with_protected_secret(key, value);
        }
        host
    };
    Ok(host)
}
