# 65 [R]: Generic OIDC providers (deferred)

**What to build:** Discovery-based support for self-hosted and enterprise identity providers — Okta, Auth0, Keycloak, Azure AD B2C — so a deployment can point at a conforming OIDC issuer without a code change. The framework is ready for this: `ClaimSource::IdToken`, nonce binding, `EmailTrust`, and the JWKS seam already exist, and OpenID Connect standardizes the claim set (`sub`, `email`, `email_verified`, `name`, `picture`), so a generic descriptor could fix the claim mapping and let discovery supply the endpoints, issuer, and JWKS URI. Discord and GitLab stay bespoke because they are not OIDC.

**Status:** deferred — do not start without settling the trust model. The blockers are decisions, not code:

- Which issuers a host may accept, and who decides. This is the hard part: accepting a package-declared issuer unchecked would let an application point the host at an attacker-controlled issuer.
- Whether packages may declare providers at all. `studio-package` admission is first-party-only by design; a package-supplied provider descriptor is an attack surface unless there is an authority that signs it.
- How scope ceilings work when the provider is not first-party. The catalog installs ceilings before descriptors today; a generic provider has no ceiling owner.

- [ ] Trust-model decision recorded: who may declare an issuer and how the set is bounded
- [ ] Scope-ceiling ownership decided for non-first-party providers
- [ ] Discovery design recorded: fetch, validate, and require the discovered issuer to equal the configured one exactly
- [ ] Generic descriptor admitted to the catalog without weakening any first-party ceiling