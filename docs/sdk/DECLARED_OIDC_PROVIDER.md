# Declaring an OpenID Connect issuer

An application that signs in with a self-hosted or enterprise identity provider — Okta, Auth0,
Keycloak, Azure AD B2C, anything that publishes a conforming discovery document — can declare
that issuer in its signed manifest instead of waiting for the host to add a maintained provider
for it.

## Declaring it

The integration id is `oidc-` followed by a name of your choosing, and the version is `1.0.0`:

```json
{
  "integrations": [
    {
      "id": "oidc-acme",
      "version": "1.0.0",
      "config": {
        "issuer": "https://acme.okta.com",
        "clientId": "acme-studio-app",
        "scopes": ["openid", "profile", "email"]
      }
    }
  ]
}
```

The capability the guest asks for is `oidc-acme.oauth`, and the operation is `sign_in`, exactly
as for a maintained provider.

| Key | Meaning |
| --- | --- |
| `issuer` | The issuer URL. It must be HTTPS. The host compares it with the issuer the discovery document publishes. |
| `clientId` | The public client id. A declared issuer is a public client: **there is no client secret**, and nothing for the developer to keep. |
| `scopes` | Optional. Every scope must be one of `openid`, `profile`, `email`, `offline_access`. |

## What the host does at launch

1. It fetches `{issuer}/.well-known/openid-configuration` over its certificate-validating HTTPS
   client, with a ten-second timeout.
2. It pins the issuer. The document must name the declared issuer, every endpoint it publishes
   must be an HTTPS URL under the issuer's own origin, and the descriptor carries the issuer
   string the document itself published, so the ID token is checked against exactly that.
3. It builds a descriptor from the published endpoints and the standard claim set: `sub`
   identifies the subject, and `name`, `preferred_username`, `email`, `email_verified`, and
   `picture` carry the profile. The address is surfaced only when the issuer says it is
   verified.
4. It installs the scope ceiling, then the descriptor, and enables the provider on the manager.

A discovery document that names another issuer, or an endpoint outside the issuer's origin, is
refused, and the launch fails closed rather than leaving the application with a provider it
declared but cannot use.

## What it does not do

- **No secret, and no scope beyond the standard set.** The host grants only the four OpenID
  Connect standard scopes, so a declared issuer's own API scopes are not reachable from the host.
- **No route group.** A declared issuer enables sign-in. The host does not know the issuer's API
  surface, so it grants no REST access to it.
- **No trust in the issuer.** A declared issuer is a public client whose redirect URI is the
  loopback listener or the broker, and the host verifies the ID token against the key set the
  discovery document published.

## The trust model

The application's signature is the authority. The operator's trust store decides which publishers'
applications may run at all, and an application from a trusted publisher is trusted code: letting
it name an issuer grants it nothing it could not already do with its own network access. The host
still owns the scope ceiling, the claim mapping, and the token verification, and it pins the
issuer rather than trusting the document it fetched.

## The redirect URI to register

Register the same redirect URI you would for a maintained provider: `http://127.0.0.1/oauth/callback`
if the issuer accepts a loopback with a varying port, or `https://{broker-host}/cb/{app-id}` if it
demands an exact HTTPS redirect. See [PROVIDER_LIVE_SETUP.md](PROVIDER_LIVE_SETUP.md).
