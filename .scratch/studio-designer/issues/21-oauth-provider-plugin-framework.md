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

- `cargo test --locked -p studio-oauth -p studio-github -p studio-package` passes, including the manager flow through the real TCP loopback listener, protected client/token storage, and `GithubHttpsOAuthTransport` over a deterministic HTTPS client.
- `cargo clippy --locked -p studio-oauth -p studio-github -p studio-package --all-targets -- -D warnings` passes.
- GitHub descriptor `1.1.0` now uses PKCE plus a protected confidential-client secret; the signed manifest carries only the secret name.
- Live GitHub verification remains open: `studio-app` has no production `HttpsClient` wiring and this environment has no disposable GitHub app credentials. Callback registration uses `http://127.0.0.1/oauth/callback`; the running listener selects the port.
