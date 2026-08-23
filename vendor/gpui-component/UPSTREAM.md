# Upstream provenance

- Project: `https://github.com/longbridge/gpui-component`
- Revision: `e1570bdc8fd2dc17d38cab09e74b1783bdf3b24b`
- Upstream package version: `0.5.2`
- License: Apache-2.0

Studio vendors the complete upstream UI crate plus its two local build-time dependencies:
`gpui-component-assets` and `gpui-component-macros`. The imported source is kept under
`vendor/` so Studio can pin GPUI and audit the platform feature set independently of upstream.

## Studio deltas

- Replaced upstream workspace inheritance with explicit dependency versions.
- Pinned all GPUI crates to Studio's Zed revision
  `381953d44897c53c4d252ae30620bafaa7d060b7`.
- Set `default-features = false` and enabled only GPUI's `wayland` feature. X11/XWayland is not
  included in the current runtime graph.
- Disabled every optional component feature by default.
- Marked the three vendored packages `publish = false` and assigned Studio-only package versions.

No UI behavior, rendering algorithms, or component APIs were rewritten. Studio's renderer owns
the protocol-to-component mapping in `crates/studio-app`.
