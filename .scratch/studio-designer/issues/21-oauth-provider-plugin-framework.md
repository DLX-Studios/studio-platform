# 21 [R]: OAuth provider-plugin framework (GitHub first)

**What to build:** First-party maintained provider plugins as versioned declarative descriptors covering endpoints, scopes, PKCE or confidential-client behavior, profile mapping, and quirks (GitHub: no refresh tokens, private email fallback). A package enables a provider by declaration with its client ID; the client secret arrives through protected configuration. The host executes browser handoff and loopback callback capture, token exchange, and refresh, storing tokens in protected storage and exposing only approved claims, status, and action results.

**Blocked by:** 18, 19

**Status:** implemented-pending-live-GitHub-verification

- [ ] Complete sign-in flow succeeds against live GitHub with tokens never present in guest memory or logs
- [x] Loopback callback listener is host-owned; guests bind nothing
- [x] Unknown, outdated, or revoked providers fail safely instead of degrading to generic network use
- [x] Shipping a new descriptor version changes flow behavior without rebuilding authored applications
- [x] Revocation clears stored tokens and the application observes revoked state

## Verification notes

- `cargo test --locked -p studio-oauth -p studio-github -p studio-package` passes, including the manager flow through the real TCP loopback listener, protected client/token storage, and `HttpsOAuthTransport` over a deterministic HTTPS client.
- `cargo clippy --locked -p studio-oauth -p studio-github -p studio-package --all-targets -- -D warnings` passes.
- GitHub descriptor `1.1.0` now uses PKCE plus a protected confidential-client secret; the signed manifest carries only the secret name.
- Live GitHub verification remains open: `studio-app` has no production `HttpsClient` wiring and this environment has no disposable GitHub app credentials. Callback registration uses `http://127.0.0.1/oauth/callback`; the running listener selects the port.

- The catalog now carries eight maintained descriptors: `apple`, `discord`, `facebook`, `gitlab`, `github`, `google`, `linkedin`, `microsoft`. Each installs its own scope ceiling before its descriptor. The adapter was renamed `HttpsOAuthTransport` once it stopped hardcoding GitHub's `accept` media type.
- OpenID Connect providers (Apple, LinkedIn, Microsoft) are verified in the host: the host fetches the provider's published JWKS over the certificate-validating client, checks RS256 against the key the token names, and asserts issuer, audience, expiry, subject, and a host-generated nonce on fresh authorizations. Apple additionally returns its name once in a `form_post` body, which the loopback listener parses with a bounded read.
- Live verification remains open for every provider, not only GitHub: this environment has no disposable OAuth applications for any of them.
- `jsonwebtoken` is pinned to the `aws_lc_rs` backend because that is what `surrealdb-core` already selects. Cargo unifies features across a workspace build, so requesting `rust_crypto` here would enable *both* backends and jsonwebtoken panics at runtime with "Could not automatically determine the process-level CryptoProvider". This only reproduces under `cargo test --workspace`, not under `-p studio-oauth`, so the ID token tests must be exercised from the workspace suite.
