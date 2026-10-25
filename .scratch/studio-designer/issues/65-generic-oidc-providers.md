# 65 [R]: Generic OIDC providers

**What to build:** Discovery-based support for self-hosted and enterprise identity providers —
Okta, Auth0, Keycloak, Azure AD B2C — so a deployment can point at a conforming OIDC issuer
without a code change. The framework is ready for this: `ClaimSource::IdToken`, nonce binding,
`EmailTrust`, and the JWKS seam already exist, and OpenID Connect standardizes the claim set
(`sub`, `email`, `email_verified`, `name`, `picture`), so a generic descriptor could fix the
claim mapping and let discovery supply the endpoints, issuer, and JWKS URI. Discord and GitLab
stay bespoke because they are not OIDC.

**Decision — the developer declares the issuer.** A signed application may name an OpenID
Connect issuer, and the host pins it rather than trusting it:

- **The application's signature is the authority.** The operator's trust store decides which
  publishers' applications may run at all, and an application from a trusted publisher is
  trusted code: letting it name an issuer grants it nothing it could not already do with its own
  network access. A declared issuer never widens what the host accepts — the host still owns the
  scope ceiling, the claim mapping, and the token verification.
- **The host pins the issuer three ways.** The discovery document must name exactly the declared
  issuer, every endpoint the document publishes must be an HTTPS URL under the issuer's own
  origin, and the ID token's `iss` must equal the issuer the document itself published.
- **The host owns the scope ceiling.** A declared issuer is offered the scopes OpenID Connect
  standardizes — `openid`, `profile`, `email`, `offline_access` — and nothing else. A declared
  issuer's own bespoke API scopes are not reachable from the host.
- **A declared issuer is a public client.** The developer holds a client id and no secret, so
  there is nothing for them to keep, and the exchange is bound by PKCE.

**Shape:** `studio-package` admits a declared issuer from the signed manifest and carries it in
the admission plan; `studio-oauth` builds the runtime descriptor from the discovery document and
fetches that document over the host's certificate-validating client; `studio-app` wires the two
together at launch.

**Status:** in progress

- [x] Trust-model decision recorded: who may declare an issuer and how the set is bounded
- [x] Scope-ceiling ownership decided for non-first-party providers
- [x] Discovery design recorded: fetch, validate, and require the discovered issuer to equal the
      configured one exactly
- [x] A signed manifest may declare one issuer, and admission refuses anything else
- [x] The host fetches discovery, pins the issuer, and admits only the standard scopes
- [x] The descriptor maps the standard claim set and verifies the ID token against the
      discovered key set
- [x] A declared issuer signs in end to end, with no client secret anywhere
- [x] The guide is written: `docs/sdk/DECLARED_OIDC_PROVIDER.md`

**Outcome.** A signed application declares an issuer with an `oidc-` integration id and an
`issuer` in its configuration. Admission synthesizes a public, identity-only descriptor for it,
refuses a scope outside the OpenID Connect standard set, refuses a non-HTTPS issuer, refuses a
configuration that names a secret, refuses a declared id that would shadow a maintained provider,
and grants it no route group. The host fetches the discovery document at launch, pins the issuer
three ways, and fails the launch closed when it cannot. The exchange is a public-client PKCE
exchange: there is no client secret for the developer to keep.
