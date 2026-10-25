# 61 [R]: Deterministic flow tests for the descriptor-only providers

**What to build:** `provider_flow.rs` runs end-to-end sign-in flows for github, google, facebook, and apple against deterministic fake upstreams. Discord, GitLab, LinkedIn, and Microsoft are descriptor-tested only. Add fake upstream routes and flow tests for those four. Discord and GitLab run through the profile-endpoint claim source; LinkedIn through a signed RS256 ID token served from the JWKS seam; Microsoft through the per-package issuer override, the one provider whose issuer comes from `ProviderPackage::with_issuer` rather than the descriptor. Microsoft's test must prove the verifier cache is keyed by resolved issuer: two packages sharing one descriptor but declaring different tenant issuers must assert against their own issuer and never accept each ot…

**Outcome:** every maintained provider now has an end-to-end flow. The JWKS seam serves one key set per published issuer, each under its own key id, so a token signed for one tenant is never accepted by another. Discord and GitLab run through the profile endpoint; LinkedIn, Microsoft, and Spotify run through signed ID tokens; Microsoft runs through the package-declared tenant issuer, and two tenants over one key-set source are proven to assert against their own issuer by the fetch count. A token bound to a nonce the host never sent, or issued by another tenant, fails with `oauth.id_token.invalid`.

**Blocked by:** 21

**Status:** done

- [x] Discord flow maps `id`/`username`/`global_name`/`email` and enforces the verification rule on both branches: address withheld when Discord has not verified it, surfaced when it has
- [x] GitLab flow maps `id`/`username`/`name`/`avatar_url`/`web_url` with the provider's own rule
- [x] LinkedIn verifies an RS256 ID token against the JWKS seam and maps `sub`/`name`/`email`
- [x] Microsoft verifies against the package-declared issuer; two tenants sharing one descriptor never accept each other's tokens, proven through verifier-cache fetch counts
- [x] A token signed for the wrong issuer, or bound to a nonce the host never sent, fails with `oauth.id_token.invalid`
- [x] All nine maintained providers have end-to-end flow coverage in `provider_flow.rs`
