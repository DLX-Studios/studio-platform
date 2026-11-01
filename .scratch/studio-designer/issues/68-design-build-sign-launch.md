# 68 [R/D]: Connect a saved Design revision to signed build and Runtime launch

**Status:** open — High. **Blocked by:** 66; preview parity 67 for native acceptance.
**Source:** Designer decisions 04/05/10, audit F03, backlog C004–C006.

The native Designer can project/edit a Design but has no audited production path to build, sign,
and launch that revision. The CLI compiler, archive packer, signatures and Runtime admission
already exist and must be reused. The contract takes an immutable Design revision plus Library
snapshot, declared manifest/target and an opaque host signing configuration reference. Define
exact asset/key matching, stable identity/events/navigation, diagnostic mapping and last-good
artifact behavior before implementing the bridge.

## First failing regression

After specifying the closed request/result contract, add a `design_build_launch` integration
fixture. Author a real Design through DesignerSession, build it twice, compare archive bytes,
verify the signature, and admit that exact artifact through StudioHost. Do not substitute a WAT
fixture or independently authored Studio Script package for the Design under test.

## Acceptance

- [ ] One validated projection feeds native preview and production lowering with stable identities.
- [ ] Identical Design/Library/target/signing inputs produce byte-identical signed artifacts.
- [ ] Referenced assets match manifest keys exactly; invalid binding/asset failures link to source.
- [ ] Tampered signature/asset, unknown capability and invalid output fail Runtime admission.
- [ ] Native Build/Run controls invoke the bridge, expose safe failures and preserve last-good output.
- [ ] Source/session/OS signing material never enters guest messages, logs, or source files.
- [ ] The launched app's state/input/navigation match the authored app, including cold reopen.

Expected scope: a shared Design build seam, existing `studio-script`/`studio-package`/host
admission, Designer UI commands, and integration tests. Separate any multi-file cross-cutting
contract work into the C004/C005/C006 slices in [completion tasks](../completion-tasks.md).

## Build contract — 2026-11-01

C004 is specified in [Design build contract v1](../../../docs/development/DESIGN_BUILD_CONTRACT.md).
The backend/native commands and required signed-artifact acceptance remain open (C005/C006).
