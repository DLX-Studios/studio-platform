# 64 [R]: Provider credential provisioning and live verification

**What to build:** Live sign-in evidence for the maintained providers. Every provider is verified
against deterministic fakes only; no live OAuth exchange has been exercised for any of them. With
60 and 63 landed the per-provider checklist is uniform: register an OAuth application at the
provider's developer portal; register the redirect URI in the shape that provider accepts; put the
client id in the manifest and the client secret into protected configuration under
`{provider}.oauth.client_secret`. Record evidence per provider: status codes and safe
`oauth.*`/`net.*` result codes only, never authorization URLs, callback values, or tokens.

**The step-by-step list is in [docs/sdk/PROVIDER_LIVE_SETUP.md](../../../docs/sdk/PROVIDER_LIVE_SETUP.md).**
It names, per provider, the artifact to create, the exact redirect URI to register, the scopes
the host will ask for, and the environment variable that carries the secret.

**What the research settled.** The redirect rules are no longer open questions:

- github: a loopback URL's **port is not compared**; the path is. `http://127.0.0.1/oauth/callback`
  works, so GitHub never needs the broker.
- google: a Desktop-app client accepts a loopback with any port. Google is deprecating the
  loopback flow for *mobile* clients, not for desktop.
- apple: the return URL must be HTTPS on a named domain, so the broker is the only path. It also
  needs a Services ID and a `.p8` key.
- microsoft: the port is ignored for a localhost URI, and HTTP is accepted only for localhost. A
  **confidential** client needs HTTPS, so the broker is the path. A *Mobile and desktop
  applications* registration would accept `http://localhost/oauth/callback` with any port, but the
  Azure portal refuses an `http` + `127.0.0.1` entry in the text box; it has to be added through
  the application manifest's `replyUrlsWithType`.
- facebook: exact match, and Meta does not accept a loopback URI for a web app. The broker is the
  path.
- discord: the port is not compared; the path is. `http://127.0.0.1/oauth/callback` works.
- gitlab: exact match, or a relative path GitLab resolves against the app's domain. The broker is
  the path.
- linkedin: exact match and HTTPS. The broker is the path.
- spotify: a loopback **IP literal** with no port, and `localhost` is refused outright. It accepts
  a dynamic port, so `http://127.0.0.1/oauth/callback` works and the broker is not needed.

**Blocked by:** 60, 62, 63

**Status:** blocked on external credentials only. Tickets 60, 62, and 63 are landed, so the host
signs in with any of the nine maintained providers, through a broker when the provider demands
an exact HTTPS redirect URI. What remains is work only the operator can do: register an OAuth
application at each provider's developer portal and supply the client ids and secrets.

- [ ] Live sign-in succeeds for each provider with tokens never present in guest memory or logs
- [ ] Evidence recorded per provider using only status codes and safe result codes
- [x] Open questions resolved and recorded: microsoft confidential-client loopback, facebook/linkedin production HTTPS, discord/gitlab loopback acceptance
- [ ] `docs/sdk/GITHUB_VIEWER.md` verification boundary updated from "deterministic fakes only" to per-provider live evidence
