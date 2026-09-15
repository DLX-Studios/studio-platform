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
| `github` | `1.0.0` | `/user`, `/user/repos`, `/repos/{owner}/{repo}` (GET, OAuth session) |
| `ai` | `1.0.0` | `/v1/chat/completions` (POST) and `/v1/chat/completions/stream` (GET), protected API key |

Provider descriptors are host policy. A package cannot add an origin, route, scope, or credential
mapping by declaring extra strings in its manifest.
