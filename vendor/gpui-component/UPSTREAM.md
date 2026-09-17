# Upstream provenance

- Project: `https://github.com/longbridge/gpui-component` (now `https://github.com/longbridge/gpui-kit`)
- Revision: `fb26e617da3add2ce2ac92a2ccc1a64bc8343135`
- Upstream package version: `0.6.1`
- License: Apache-2.0

Studio vendors the complete upstream UI crate plus its required sibling crates:
`gpui-base` (behavior/interaction foundations), `gpui-kit-assets` (default icon
bundle), and `gpui-component-macros` (procedural macros). The imported source is kept under
`vendor/` so Studio can pin GPUI and audit the platform feature set independently of upstream.

## Studio deltas

- Replaced upstream workspace inheritance with explicit dependency versions.
- Replaced the upstream `gpui-pre`/`gpui-pre-macros`/`gpui-pre-sum-tree` crates.io
  requirements with the exact reviewed versions (`0.3.5`/`0.3.5`/`0.3.5`), matching
  the workspace-wide GPUI pin below.
- Pinned all GPUI crates to `gpui-pre 0.3.5` (Zed snapshot `d89e9c2`, per the
  published crate description).
- Set `default-features = false` and enabled only GPUI's `wayland` feature. X11/XWayland is not
  included in the current runtime graph.
- Disabled every optional component feature by default.
- Renamed the vendored assets package directory to `vendor/gpui-kit-assets` to follow the
  upstream `gpui-kit-assets` package name; `vendor/gpui-base` is new for the 0.6.x
  component/base split.
- Marked the four vendored packages `publish = false` and assigned Studio-only package versions
  (`0.6.1-studio.1`).

No UI behavior, rendering algorithms, or component APIs were rewritten. Studio's renderer owns
the protocol-to-component mapping in `crates/studio-app`.
