# 36 [R]: GitHub viewer proof application

**What to build:** Small OAuth proof application preceding the flagship journey: sign in through the GitHub provider plugin, list repositories via authenticated declared REST routes, view a repository detail screen. Packaged, signed, launched in real Runtime. Establishes the `@studio/github` SDK package shape and the `@studio/ai` OpenAI-compatible streaming client skeleton for later use.

**Blocked by:** 21, 23

**Status:** ready-for-agent

- [ ] End-to-end sign-in and repo listing succeed against live GitHub
- [ ] Requests outside declared routes are denied even with valid session
- [ ] Package builds deterministically and launches in Runtime
- [ ] Documentation shows adding a second provider requires no application changes

## Implementation notes

- Added `crates/studio-github` with the versioned GitHub provider descriptor, host-resolved OAuth
  route groups, typed `/user`, `/user/repos`, and `/repos/{owner}/{repo}` projections, and a
  host-neutral sign-in → repository list → repository detail state model.
- Added `crates/studio-ai` plus the `sdk/ai` package with an OpenAI-compatible request/chunk shape;
  API keys remain named protected configuration and SSE chunks arrive through the broker stream.
- Added `sdk/github` (`@studio/github`) and a signed `examples/github-viewer` package. Manifest
  integration and route declarations are now first-class package fields and are revalidated against
  broker ceilings before admission.
- Live OAuth/API credentials, callback capture, and native Runtime launch were not exercised in
  this implementation pass; those external gates remain unchecked.
