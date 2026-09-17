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
- Audited revision: `fb26e617da3add2ce2ac92a2ccc1a64bc8343135` (upstream package `0.6.1`)
- License: Apache License 2.0
- Current use: the full upstream component library (Button, Input, Select, Slider, and
  companion controls) plus the required `gpui-base`, `gpui-kit-assets`, and
  `gpui-component-macros` crates, vendored under `vendor/` with Wayland-only GPUI
  declarations and exposed exclusively behind Studio-owned wrappers and the
  protocol-to-component mapping in `crates/studio-app`.
- Fork details: `docs/upstream/gpui-component-delta.md`

## GPUI (`gpui-pre` distribution)

- Project: https://github.com/zed-industries/zed, distributed as `gpui-pre 0.3.5`
  (Zed snapshot `d89e9c2`) via crates.io
- License: Apache License 2.0, as declared by the published distribution; distribution
  review must still confirm the applicable license of the linked GPUI crates and resulting
  Studio binary before release.
- Current use: native Wayland rendering/platform backend with default features disabled.

## rsvelte

- Project: https://github.com/baseballyama/rsvelte
- Pinned revision: `b64aed9c611557649ebd6428455cfe874255356e`
- License: MIT License, copyright Yuichiro Yamashita; the notice ships in
  `vendor/rsvelte/LICENSE` per the license terms.
- Current use: vendored Svelte-compatible parser crates (`rsvelte`, `rsvelte_core`,
  `rsvelte_esrap`, plus manifest-only `rsvelte_ast_equiv` and `rsvelte_projection`)
  used exclusively behind the `studio-script/src/rsvelte_adapter.rs` boundary as the
  `.studio` source frontend. Generated JavaScript output is never used.
- Upstream details: `vendor/rsvelte/UPSTREAM.md`

## lucide-static

- Project: https://github.com/lucide-icons/lucide (package `lucide-static@1.40.0`)
- License: ISC License.
- Current use: build-time icon source only. The toolchain collects referenced SVGs
  from the pinned package into bundles; committed `assets/icons/` files override
  package files. No lucide code ships in binaries.

All remaining Rust and Bun packages are lockfile-pinned registry dependencies. Their license texts
and notices must be collected by the release packaging job; see `docs/security/DEPENDENCY_AUDIT.md`.
