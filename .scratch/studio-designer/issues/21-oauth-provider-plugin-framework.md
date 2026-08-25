# 21 [R]: OAuth provider-plugin framework (GitHub first)

**What to build:** First-party maintained provider plugins as versioned declarative descriptors covering endpoints, scopes, PKCE or confidential-client behavior, profile mapping, and quirks (GitHub: no refresh tokens, private email fallback). A package enables a provider by declaration with its client ID; the client secret arrives through protected configuration. The host executes browser handoff and loopback callback capture, token exchange, and refresh, storing tokens in protected storage and exposing only approved claims, status, and action results.

**Blocked by:** 18, 19

**Status:** ready-for-agent

- [ ] Complete sign-in flow succeeds against live GitHub with tokens never present in guest memory or logs
- [ ] Loopback callback listener is host-owned; guests bind nothing
- [ ] Unknown, outdated, or revoked providers fail safely instead of degrading to generic network use
- [ ] Shipping a new descriptor version changes flow behavior without rebuilding authored applications
- [ ] Revocation clears stored tokens and the application observes revoked state
