# 64 [R]: Provider credential provisioning and live verification

**What to build:** Live sign-in evidence for all eight maintained providers. Every provider is verified against deterministic fakes only; no live OAuth exchange has been exercised for any of them, including github. With 60 and 63 landed the per-provider checklist is uniform: register an OAuth application at the provider's developer portal; register the broker URL as the redirect (or a loopback URI when verifying an RFC 8252 provider offline); request exactly the descriptor's scopes; place the client id in the manifest and the client secret into protected configuration under `{provider}.oauth.client_secret`. Record evidence per provider: status codes and safe `oauth.*`/`net.*` result codes only, never authorization URLs, callback values, or tokens.

Per-provider registration prerequisites and open questions to resolve during this ticket:

- github: OAuth App; loopback callback with any port is accepted for offline verification.
- google: desktop OAuth client; loopback redirect with any port is documented as accepted.
- apple: Services ID plus a Sign in with Apple key and domain verification; the return URL must be HTTPS, so the broker is the only path.
- microsoft: app registration; the tenant issuer is declared via `withIssuer`. Open question: whether a confidential client accepts loopback HTTP or requires an HTTPS redirect.
- facebook: exact-match redirect. Open question: whether production mode accepts loopback HTTP or requires HTTPS.
- discord, gitlab, linkedin: exact-match redirects; verify per provider whether loopback HTTP is accepted outside development mode.
- spotify: blocked on 62.

**Blocked by:** 60, 62, 63

**Status:** blocked on external credentials and the tickets above

- [ ] Live sign-in succeeds for each provider with tokens never present in guest memory or logs
- [ ] Evidence recorded per provider using only status codes and safe result codes
- [ ] Open questions resolved and recorded: microsoft confidential-client loopback, facebook/linkedin production HTTPS, discord/gitlab loopback acceptance
- [ ] `docs/sdk/GITHUB_VIEWER.md` verification boundary updated from "deterministic fakes only" to per-provider live evidence