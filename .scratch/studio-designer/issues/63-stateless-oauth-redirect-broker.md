# 63 [R]: Stateless OAuth redirect broker

**What to build:** A deployable, self-hostable, stateless HTTPS service that translates a provider's redirect into a loopback redirect, so providers that demand an exact registered HTTPS redirect URI — apple, facebook, discord, gitlab, linkedin, and spotify — work without a fixed local port. The broker holds nothing: no database, no disk, no retention beyond the response, no logs of codes or states.

Mechanics: the app binds its ephemeral loopback listener as today and embeds the bound port inside the host-generated `state` (opaque and unguessable to providers, echoed back verbatim). The developer registers the broker URL `https://{host}/cb/{app-id}` as the provider's redirect URI and pastes it once per provider. The broker accepts the provider redirect as a GET query or an Apple `form_post` body, validates the `state` shape, extracts only the loopback port, and responds `302` to `http://127.0.0.1:{port}/oauth/callback?code=...&state=...` carrying the provider's payload fields. Everything downstream — PKCE, state verification, token exchange, protected storage — is unchanged and stays local.

Hard rules:

- The broker never accepts a redirect target from `state`. Host and path are hardcoded; only a validated loopback port may vary. A future mobile deep-link hop is a second sanctioned target shape behind the same allowlist, never a free-form URL. An open redirector is worse than any port problem.
- Zero persistence: codes are single-use and expire in minutes at the provider regardless; the broker must not log or retain them.

Client-side adjustments: `try_sign_in` compares the listener redirect URI against the authorize redirect URI today; with the broker those differ, so the binding check becomes "the state's embedded port matches the bound listener." The loopback listener must accept the descriptor-named one-time payload field (`user` for apple) in the query, since a 302 converts the form POST to a GET. Loopback-only mode remains available as host configuration so offline use of RFC 8252 providers (github, google) never depends on the broker. The browser must be co-located with the app for the final loopback hop; a remote-browser scenario is out of scope and would need a different design.

**Blocked by:** 21

**Status:** ready-for-agent (service deployment owned by Sir White; spec must be deployable without touching this repository)

- [ ] Broker implemented to this spec and deployable self-hosted with no platform state
- [ ] GET and `form_post` POST redirects translate to a loopback 302 carrying code, state, and provider payload fields
- [ ] `state` carrying a full URL, a non-loopback host, or an out-of-range port is refused with a safe response
- [ ] Apple end-to-end through the broker: form POST parsed, one-time name relayed and composed into approved claims
- [ ] Loopback-only offline mode keeps github and google working with the broker absent
- [ ] No code, state, or token value appears in broker logs, disk, or memory after the response