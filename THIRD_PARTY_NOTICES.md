# Third-Party Notices

Studio Runtime is expected to incorporate and adapt selected open-source dependencies. This file records source-code reuse in addition to dependency license metadata.

## Oxide

- Project: https://github.com/niklabh/oxide
- Audited revision: `29cd89882465d6ebfe00af2ada6f89951581c580`
- License: Apache License 2.0
- Current use: architectural reference only. The final audit found no copied or adapted Oxide
  source; independently implemented areas and reviewed references are listed in
  `docs/upstream/oxide-audit.md`.

## gpui-component

- Project: https://github.com/longbridge/gpui-component
- Audited revision: `e1570bdc8fd2dc17d38cab09e74b1783bdf3b24b`
- License: Apache License 2.0
- Current use: the minimal animation/easing foundation from `crates/ui/src/animation.rs`, adapted
  for lint documentation only and exposed exclusively behind Studio-owned wrappers.
- Upstream source SHA-256: `8a7d3fc06579f5c43cacebb454d5a9611d528c697ac63038bae9f7348f18e5f7`
- Fork details: `docs/upstream/gpui-component-delta.md`

## Zed GPUI

- Project: https://github.com/zed-industries/zed
- Pinned revision: `381953d44897c53c4d252ae30620bafaa7d060b7`
- License: GPL-3.0-or-later for the upstream repository; distribution review must confirm the
  applicable license of the linked GPUI crates and resulting Studio binary before release.
- Current use: native Wayland rendering/platform backend with default features disabled.

All remaining Rust and Bun packages are lockfile-pinned registry dependencies. Their license texts
and notices must be collected by the release packaging job; see `docs/security/DEPENDENCY_AUDIT.md`.
