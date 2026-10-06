# GitHub viewer proof application

The checked-in `examples/github-viewer` package is the smallest signed Runtime launch target for
the provider-plugin path. Its manifest pins the `github` integration descriptor and signs three
REST route groups. The viewer uses a confidential GitHub OAuth app with host-owned S256 PKCE; it
requests only `read:user` and `user:email`. The manifest contains the public client ID and a name
for the protected client-secret entry, never the secret value itself:

| route group | method | path | credential |
| --- | --- | --- | --- |
| `github.user` | `GET` | `/user` | host-resolved GitHub session |
| `github.repositories` | `GET` | `/user/repos` | host-resolved GitHub session |
| `github.repository` | `GET` | `/repos/{owner}/{repo}` | host-resolved GitHub session |

`crates/studio-github` is the host-neutral typed SDK. `GithubClient` accepts only the restricted
`GuestRestApi`; it cannot receive an OAuth token. `GithubViewer` models the deterministic journey:
sign-in request, authenticated repository list, and repository detail. `GithubGuestEvent` and
`GithubHostEvent` are the closed event contract for those transitions; their payloads contain only
approved profile/repository projections. Browser handoff, callback capture, token storage, and
send-time credential injection remain host responsibilities.

`crates/studio-ai` and `sdk/ai` establish the provider-neutral OpenAI-compatible request and
validated SSE chunk shape. Streaming uses a declared, bounded POST adapter route carrying the
same ordered messages, temperature, and `stream_options` body as a non-streaming request;
applications do not gain a raw socket or an API-key path.

## Build and launch

With Bun and the pinned AssemblyScript toolchain installed:

```bash
bun run ./scripts/build-example.ts github-viewer
cargo run -p studio-app -- --dev examples/github-viewer/build/github-viewer.studio
```

Before a production launch, replace the manifest's example client ID and configure the GitHub OAuth
client secret in protected host storage under `github.oauth.client_secret`. Register
`http://127.0.0.1/oauth/callback` as the loopback callback base; the host binds a fresh
`127.0.0.1` port for each sign-in. GitHub permits the runtime redirect to use that port. The host
validates the exact callback path and state and discards malformed, denied, replayed, or
scope-mismatched callbacks before token exchange.

## Staging evidence

The deterministic provider-flow tests run with:

```text
cargo test --locked -p studio-oauth -p studio-github -p studio-package
```

They use the real host-owned TCP loopback listener and protected token store with
`HttpsOAuthTransport` over a deterministic HTTPS client. They verify PKCE callback/state
validation, protected-secret exchange and GitHub revocation, minimal scopes, private-email fallback,
latest-descriptor updates, send-time token injection, and fail-closed refresh behavior. The same
adapter and manager then run the other maintained providers end to end. Google asserts the
descriptor's authorization parameters and profile `accept` type reach the wire, that claims map to
the approved set, that a refresh token rotates the session, and both branches of its email trust
rule. Facebook asserts the descriptor-supplied `fields` query and a nested `picture.data.url` claim
path resolve, and both branches of the provider-supplied-email flag. Apple signs a real RS256 ID
token against a generated key, served through the JWKS seam: the flow asserts the descriptor's
`response_mode` reached the authorization URL, that the one-time `user` payload is parsed from a
form POST body and composed into a display name, that the signed subject and address are used while
the unsigned callback data only fills what the token omits, that one key-set fetch serves both
sign-in and refresh, and that a correctly signed token bound to a *different* nonce is rejected.
This is not evidence of a live exchange with any provider.

A live staging run needs a disposable GitHub OAuth app with its client ID in the signed manifest.
Inject `STUDIO_GITHUB_CLIENT_SECRET` through the deployment secret manager; `studio-app` persists it
in the OS protected store under `github.oauth.client_secret` and removes it from the browser
launcher environment. `studio-app` supplies the GPUI HTTPS client to `HttpsOAuthTransport`.
Record the live run with:

```text
STUDIO_GITHUB_CLIENT_SECRET=... \
<host launch command>
```

The evidence record should include only status codes and safe `oauth.*`/`net.*` result codes—never
authorization URLs, callback query values, access tokens, refresh tokens, or raw upstream bodies.

## Adding a second provider

Applications depend on the generic `GithubRestClient`/`RestRequest` and `AiTransport` boundaries,
not provider-specific sockets or credential code. A second integration contributes another
versioned descriptor, package integration reference, and route groups. The viewer's application
state and typed screen journey remain unchanged; only the selected integration and its host
configuration change.

Eight maintained descriptors live in `ProviderRegistry::maintained()`: `apple`, `discord`,
`facebook`, `gitlab`, `github` `1.1.0`, `google`, `linkedin`, and `microsoft`. The manager,
callback listener, PKCE generation, state check, protected vault, JWKS verifier, and HTTPS adapter
are shared unchanged across all eight. Adding the second through eighth required no change to
application code, the broker, or `studio-github`.

The catalog installs each provider's scope ceiling *before* its descriptor, so no maintained
descriptor can borrow another provider's grant budget, and a runtime hot-update cannot widen what an
application is offered. A provider with no installed ceiling admits nothing.

### Where claims come from

`claimSource` is either `profileEndpoint` or `idToken`. The same `profile` mapping describes where
each approved claim lives either way, so a descriptor states *which document* it reads rather than
duplicating the mapping.

For `idToken` providers the host verifies the signature itself rather than trusting the token
because it arrived over TLS: fetch the published key set over the certificate-validating client,
select the single key named by the token header, verify RS256, then assert issuer, audience,
expiry, and subject. Only RS256 is accepted, because letting a token name its own algorithm is
what allows an unsigned or symmetrically-signed token to be checked against a published RSA key.
Key sets are cached per descriptor version and issuer, and an unknown `kid` triggers one refetch
because providers publish a new key before retiring the old one.

A fresh authorization also carries a host-generated `nonce`, which the token must echo. Without it
a token captured from another session replays into this one. A refresh carries no new nonce,
because there is no new authorization request to bind against: the refresh token is the binding,
and it only exists because the original exchange passed both checks.

### Provider differences carried as data

- `authorizationParams` for Google's `access_type=offline`, without which it returns a session that
  cannot be refreshed. Descriptors may not supply `response_type`, `client_id`, `redirect_uri`,
  `scope`, `state`, `code_challenge`, `nonce`, or `response_mode`; those stay host-owned.
- A descriptor-declared profile `accept` media type, replacing an adapter that hardcoded GitHub's
  `application/vnd.github+json`.
- The `fields=` query Graph needs is part of the endpoint, so `fields=id,name,email,picture,link`
  is descriptor data rather than call-site code.
- A dotted claim path reaches Graph's nested avatar through `picture.data.url`, and Microsoft
  names the subject `oid` rather than `sub`.
- `responseMode: formPost` plus `oneTimePayloadField: user` covers Apple, which returns the
  account name exactly once, as a JSON field in a POST body. `nameComposition` then joins
  `name.firstName` and `name.lastName`. Merged values never override a signed claim, so unsigned
  callback data can only contribute a claim the provider did not sign.
- `emailTrust` covers the three real-world shapes: `Unstated` (GitLab verifies addresses itself
  and publishes no claim), `Verified` (Google's `email_verified` boolean, Apple's `"true"`
  string), and `UnverifiedWhenSet` (Facebook's `oauth_provided_email` and Discord's `verified`,
  which report the inverse). Anything short of an affirmative answer withholds the address.

### Tenants that name their own issuer

Microsoft writes the issuing tenant into `iss`, so no single maintained descriptor can name every
valid issuer. Its descriptor therefore declares no issuer, and a package states the one it expects
through `withIssuer`. The host asserts against exactly that value rather than matching a prefix,
and refuses to enable Microsoft at all until the package has declared one. An issuer override on a
profile-endpoint provider is refused outright, because there is no signature to check it against.

### Deliberately not expressed

- **Upstream revocation, for every provider except GitHub.** The adapter implements GitHub's
  authenticated JSON `DELETE`; every other provider here uses an RFC 7009 form POST. Declaring one
  would send a request the provider rejects. Local revocation still clears protected material and
  reports `Revoked`.
- **No Facebook long-lived token exchange.** `noRefreshTokens` is true because Facebook's
  `fb_exchange_token` is a separate protocol. Refresh fails closed rather than silently expiring.
- **No Discord avatar.** Discord returns a hash that only becomes a URL after being joined against
  its CDN host, which a dotted claim path cannot express. No avatar is mapped rather than
  surfacing a value that is not a URL.
- **No Spotify.** Its token endpoint authenticates the client with HTTP Basic, which the adapter
  does not implement. A descriptor for it would parse, validate, and then fail at the exchange.

## Verification boundary

The deterministic SDK and route contracts are checked in, but live OAuth, GitHub API, signing, and
Runtime launch acceptance require provider credentials, a configured callback environment, and a
native Wayland session. Those external gates remain intentionally visible until exercised.
