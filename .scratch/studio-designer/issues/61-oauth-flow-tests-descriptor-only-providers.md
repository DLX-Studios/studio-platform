# 61 [R]: Deterministic flow tests for the descriptor-only providers

**What to build:** `provider_flow.rs` runs end-to-end sign-in flows for github, google, facebook, and apple against deterministic fake upstreams. Discord, GitLab, LinkedIn, and Microsoft are descriptor-tested only. Add fake upstream routes and flow tests for those four. Discord and GitLab run through the profile-endpoint claim source; LinkedIn through a signed RS256 ID token served from the JWKS seam; Microsoft through the per-package issuer override, the one provider whose issuer comes from `ProviderPackage::with_issuer` rather than the descriptor. Microsoft's test must prove the verifier cache is keyed by resolved issuer: two packages sharing one descriptor but declaring different tenant issuers must assert against their own issuer and never accept each other's tokens.

**Blocked by:** 21

**Status:** ready-for-agent

- [ ] Discord flow maps `id`/`username`/`global_name`/`email` and enforces `EmailTrust::UnverifiedWhenSet("verified")` on both branches: address withheld when Discord has not verified it, surfaced when it has
- [ ] GitLab flow maps `id`/`username`/`name`/`avatar_url`/`web_url` with `EmailTrust::Unstated`
- [ ] LinkedIn verifies an RS256 ID token against the JWKS seam and maps `sub`/`name`/`email`
- [ ] Microsoft verifies against the package-declared issuer; two tenants sharing one descriptor never accept each other's tokens, proven through verifier-cache fetch counts
- [ ] A token signed for the wrong issuer, or bound to a nonce the host never sent, fails with `oauth.id_token.invalid`
- [ ] All eight maintained providers have end-to-end flow coverage in `provider_flow.rs`