# 63 [R]: Stateless OAuth redirect broker

**What to build:** A deployable, self-hostable, stateless service that translates a provider's
redirect into a loopback redirect, so providers that demand an exact registered redirect URI —
apple, facebook, discord, gitlab, linkedin, and spotify — work without a fixed local port. The
broker holds nothing: no database, no disk, no retention beyond the response, no logs of codes
or states.

Mechanics: the app binds its ephemeral loopback listener as today and embeds the bound port
inside the host-generated `state` (opaque and unguessable to providers, echoed back verbatim).
The developer registers the broker URL `https://{host}/cb/{app-id}` as the provider's redirect
URI and pastes it once per provider. The broker accepts the provider redirect as a GET query or
an Apple `form_post` body, validates the `state`, extracts only the loopback port, and responds
`302` to `http://127.0.0.1:{port}/oauth/callback?code=...&state=...` carrying the provider's
payload fields. Everything downstream — PKCE, state verification, token exchange, protected
storage — is unchanged and stays local.

**The `state` is authenticated, not merely parsed.** The port travels in the `state` as
`port || tag || entropy`, where the tag is `HMAC-SHA256(secret, port || entropy)` and the
secret is shared between the application and the broker. A broker that merely parsed a port out
of the `state` would let anyone aim its redirects at an arbitrary loopback service: an open
redirector wearing a loopback address. With the tag, a `state` the broker did not mint is
refused, and the port it names is one the application proved it chose. The secret is
configuration, not state — the broker still stores nothing per request, and the shared contract
lives in one crate so a client and a broker cannot drift apart.

Hard rules:

- The broker never accepts a redirect target from `state`. Host and path are hardcoded; only a
  validated loopback port may vary, and only when the `state` authenticates it. A future mobile
  deep-link hop is a second sanctioned target shape behind the same allowlist, never a
  free-form URL. An open redirector is worse than any port problem.
- Zero persistence: codes are single-use and expire in minutes at the provider regardless; the
  broker must not log or retain them.

Client-side adjustments: `try_sign_in` compares the listener redirect URI against the authorize
redirect URI today; with the broker those differ, so the binding check becomes "the state's
embedded port matches the bound listener". The loopback listener must accept the
descriptor-named one-time payload field (`user` for apple) in the query, since a 302 converts
the form POST to a GET. Loopback-only mode remains available as host configuration so offline
use of RFC 8252 providers (github, google) never depends on the broker. The browser must be
co-located with the app for the final loopback hop; a remote-browser scenario is out of scope
and would need a different design.

**Shape:** `crates/studio-oauth-redirect` holds the contract both sides obey (the state format,
the callback path, the form-encoding rules); `crates/studio-oauth-broker` is the service;
`crates/studio-oauth` is the client.

**Blocked by:** 21

**Status:** done

- [x] Shared contract crate: state format, callback path, form-encoding rules, one definition
      shared by the client and the service
- [x] Client side: the manager takes a broker, the `state` carries the bound port, the binding
      check compares that port to the listener, and the exchange sends the broker's redirect URI
- [x] Broker implemented to this spec and deployable self-hosted with no platform state
      (`crates/studio-oauth-broker`, plain HTTP behind a TLS-terminating proxy)
- [x] GET and `form_post` POST redirects translate to a loopback 302 carrying code, state, and provider payload fields
- [x] `state` carrying a full URL, a non-loopback host, or an out-of-range port is refused with a safe response
- [x] A `state` the broker did not mint is refused with 400
- [x] The Apple-shaped path end to end: form POST parsed, the one-time name relayed and composed
      into approved claims. The end-to-end proof uses a provider fixture with Apple's shape —
      form POST, a `user` payload, a composed name — because Apple's own ID token needs an RSA
      key, and Apple's flow is covered separately in `provider_flow.rs`. The proof found a real
      defect: the one-time payload was merged only into an ID token's claim document, so a
      provider whose claims come from a profile endpoint lost it. The payload now merges into
      either.
- [x] Loopback-only offline mode keeps github and google working with the broker absent
- [x] No code, state, or token value appears in broker logs, disk, or memory after the response
- [x] The host reads the broker from `STUDIO_OAUTH_BROKER_URL` and `STUDIO_OAUTH_BROKER_SECRET`,
      and a half-configured broker is a startup failure rather than a silent fallback
      (`docs/security/OAUTH_REDIRECT_BROKER.md`)
