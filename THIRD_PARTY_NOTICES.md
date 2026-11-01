# Third-Party Notices

Studio Runtime is expected to incorporate and adapt selected open-source dependencies. This file records source-code reuse in addition to dependency license metadata.

## Oxide

- Project: https://github.com/niklabh/oxide
- Audited revision: `29cd89882465d6ebfe00af2ada6f89951581c580`
- License: Apache License 2.0
- Current use: architectural reference only. The final audit found no copied or adapted Oxide
  source; independently implemented areas and reviewed references are listed in
  `docs/upstream/oxide-audit.md`.

## gpui-kit

- Project: https://github.com/longbridge/gpui-kit (formerly
  https://github.com/longbridge/gpui-component)
- Version: published crates.io `gpui-kit 0.8.1` and matching Kit layers.
- License: Apache License 2.0, as declared by the published packages.
- Current use: native controls behind Studio-owned wrappers and protocol mappings. The previous
  five-crate vendor fork was removed; Cargo.lock records the registry source and checksums.
- Dependency policy: `docs/upstream/gpui-kit-delta.md`.

## GPUI (`gpui-pre` distribution)

- Project: https://github.com/zed-industries/zed, distributed through crates.io.
- Version: `gpui-pre 0.3.10` and matching platform crates selected by GPUI Kit.
- License: Apache License 2.0 as declared by the distribution; review all linked dependencies
  and distribution obligations before release.
- Current use: upstream native backends, including Wayland and X11 on Linux and `font-kit`
  for macOS text rendering.

## uds_windows

- Project: https://github.com/haraldh/rust_uds_windows
- Version: crates.io `uds_windows 1.2.1`, locked with registry checksum.
- License: MIT, as declared by the published crate.
- Current use: Windows-only local AF_UNIX reload transport. Unix targets use the Rust standard
  library and do not build this dependency. Studio-owned code adds no unsafe implementation.

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
