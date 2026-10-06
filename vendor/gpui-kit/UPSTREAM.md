# Upstream provenance

- Project: `https://github.com/longbridge/gpui-kit` (formerly `https://github.com/longbridge/gpui-component`)
- Revision: `0c830f4d257e69fdd17200650533ab4ca9a40cc0`
- Upstream package version: `0.7.0`
- License: Apache-2.0

Studio vendors the complete upstream `gpui-kit` facade crate plus its required sibling crates:
`gpui-component` (the UI component set re-exported as `gpui_kit::component`), `gpui-base`
(behavior/interaction foundations, re-exported as `gpui_kit::base`), `gpui-kit-assets` (default
icon bundle, re-exported as `gpui_kit::assets`), `gpui-component-macros` (procedural macros), and
`gpui-platform` (re-exported as `gpui_kit::platform`). The imported source is kept under `vendor/`
so Studio can pin GPUI and audit the platform feature set independently of upstream.

## Studio deltas

- Replaced upstream workspace inheritance with explicit dependency versions.
- Replaced the upstream `gpui-pre`/`gpui-pre-macros`/`gpui-pre-sum-tree` crates.io
  requirements with the exact reviewed versions (`0.3.7`/`0.3.7`/`0.3.7`), matching
  the workspace-wide GPUI pin below.
- Pinned all GPUI crates to `gpui-pre 0.3.7`, moving the pin from `0.3.5`.
- Set `default-features = false` and enabled only GPUI's `wayland` feature. X11/XWayland is not
  included in the current runtime graph.
- Disabled every optional component feature by default.
- Added the `vendor/gpui-kit` facade crate for 0.7.0 and reduced its `gpui_platform`
  dependency to `default-features = false` with only `wayland`, so X11 is never compiled or linked.
- Marked the five vendored packages `publish = false` and assigned Studio-only package versions
  (`0.7.0-studio.1`).

No UI behavior, rendering algorithms, or component APIs were rewritten. Studio's renderer owns
the protocol-to-component mapping in `crates/studio-app`.
