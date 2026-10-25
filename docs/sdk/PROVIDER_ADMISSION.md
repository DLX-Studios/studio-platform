# Provider capability admission

Provider integrations are admitted before a guest module is instantiated. A package names an
exact integration descriptor version; the host resolves that `(id, version)` pair from its
maintained `studio-package::ProviderRegistry`. Unknown, outdated, revoked, and incompatible
descriptors fail closed.

Admission compares each signed route declaration with the resolved descriptor's origin, method,
path, allowed-header, and credential policy. Integration configuration is metadata only: client
identifiers, scopes, origins, and protected-secret names are checked without ever accepting a
credential value. Every named secret must also have a manifest declaration containing a purpose.

The result is a `ProviderAdmissionPlan` containing resolved provider versions, admitted capability
and secret names, and broker-compiled route groups. `install_into` transfers those compiled groups
to `studio-net::RestBroker`; the broker does not reinterpret manifest route strings. Diagnostics
contain only stable provider, version, route, and failure-code context, so config values and secret
material cannot enter launch errors.

The maintained first-party fixtures are:

| Integration | Version | Routes |
| --- | --- | --- |
| `github` | `1.1.0` | `/user`, `/user/repos`, `/repos/{owner}/{repo}` (GET, OAuth session) |
| `apple` | `1.0.0` | none: sign-in only |
| `discord` | `1.0.0` | none: sign-in only |
| `facebook` | `1.0.0` | none: sign-in only |
| `gitlab` | `1.0.0` | none: sign-in only |
| `google` | `1.0.0` | none: sign-in only |
| `linkedin` | `1.0.0` | none: sign-in only |
| `microsoft` | `1.0.0` | none: sign-in only |
| `spotify` | `1.0.0` | none: sign-in only |
| `ai` | `1.0.0` | `/v1/chat/completions` (POST) and `/v1/chat/completions/stream` (GET), protected API key |

An identity-only integration enables sign-in without granting any REST surface: the application
authenticates the account and reads the approved claims, but the host broker holds no route
group for that provider, so the provider's API is unreachable from the guest.

Every OAuth integration requires the protected secret `{provider}.oauth.client_secret` with the
purpose the catalog declares, and reads the client id from the integration's `clientId`. A
Microsoft package additionally states the tenant issuer it expects, because the token names the
tenant that issued it.

Provider descriptors are host policy. A package cannot add an origin, route, scope, or credential
mapping by declaring extra strings in its manifest.
