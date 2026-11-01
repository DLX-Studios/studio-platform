# 69 [D/R]: Prove one complete native authoring-to-Runtime journey

**Status:** open — blocked by implementation, not an executed acceptance pass.
**Blocked by:** 66, 67, 68, native Library and Workbench/support wiring (C007/C008/C013).
**Source:** Designer decisions 04/10/12; audit F01–F09; backlog C015.

Execute the [local acceptance journey](../../../docs/development/DESIGNER_ACCEPTANCE_JOURNEY.md)
with a fresh data directory, Alice/Bob identities, and one two-screen counter application. It
must travel through the production identity/project/session/Library/projection/build/signature/
Runtime seams with the same authored revision throughout. External cloud/provider/physical POS
acceptance remains separately required for the full v1 release.

## Acceptance

- [ ] J01–J11 pass through a composed `authoring_journey` integration target with durable reopen.
- [ ] Real native GPUI input/render tests cover the production view and authored values.
- [ ] Bob cannot open, inherit or mutate Alice's project; invalid edits/builds preserve last-good state.
- [ ] Token/profile/asset and prototype changes agree with the signed, launched Runtime artifact.
- [ ] J12 has per-platform keyboard/focus/scaling/reduced-motion/accessibility evidence.
- [ ] Evidence records commit, source/Library revision, artifact digest, safe codes and actual test layer.

Expected first test: `crates/studio-designer/tests/authoring_journey.rs` after the missing build
contract and renderer seam exist. Do not add a permanently ignored or unconditional-success test
to make the current disconnected path appear complete. Record failures as backlog items until
fixed, and never use a separately handwritten fixture app to close the signed-output step.
