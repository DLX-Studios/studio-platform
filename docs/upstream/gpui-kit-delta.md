# gpui-kit Fork Policy

Upstream revision: `0c830f4d257e69fdd17200650533ab4ca9a40cc0` (package `0.7.0`).

Audit completed: 2026-08-04 for `e1570bdc8fd2dc17d38cab09e74b1783bdf3b24b` (package `0.5.2`);
synchronized 2026-09-17 to `fb26e61` (package `0.6.1`); synchronized 2026-10-04 to `0c830f4`
(package `0.7.0`). This revision replaces the former single-crate vendor tree with the upstream
`component`/`base` split plus the required `assets` and `macros` crates, and adds the unified
`gpui-kit` facade crate that Studio now imports.

## Retained scope

Studio uses upstream components as native controls: Button is integrated in the initial renderer,
and Input, Select, and Slider are available to the runtime for protocol mapping. Layout containers
remain host-owned GPUI layout primitives because they are the native compositional API underneath
the component library.

The full source is retained rather than copied piecemeal, so component behavior, accessibility,
and visual semantics stay upstream-compatible. It also avoids maintaining local imitations of
Button, Input, Select, or Slider.

## Enabled features

- GPUI: `gpui-pre 0.3.7` (Zed snapshot `1a28cff`) with `default-features = false`, `wayland` only.
- gpui-kit: no default optional features.
- No X11/XWayland feature is enabled in the shipping dependency graph, including through the
  `gpui_platform` dependency of the `gpui-kit` facade, which upstream declares with `font-kit`,
  `x11`, `wayland`, and `runtime_shaders` and Studio overrides to `wayland` only.

The project can add an opt-in X11 backend later; it must be a separate build feature and receive
its own dependency and release-binary audit. The present runtime remains Wayland-only.

## Local deltas

- Replaced upstream workspace dependencies with explicit versions and local asset/macro paths.
- Moved GPUI off the Zed git pin onto crates.io `gpui-pre 0.3.7` (same Zed code, registry
  distribution); `gpui-pre-macros`/`gpui-pre-sum-tree` follow at `0.3.7`.
- Added Wayland-only GPUI dependency declarations in all five vendored crates
  (`gpui-kit`, `gpui-component`, `gpui-base`, `gpui-kit-assets`, `gpui-component-macros` where
  applicable).
- Added the `vendor/gpui-kit` facade crate for 0.7.0, re-exporting GPUI plus `component`, `base`,
  `assets`, and `platform`; reduced its `gpui_platform` dependency to `wayland` only.
- Adopted the upstream `gpui-kit-assets` package name (`vendor/gpui-kit-assets`) and added
  `vendor/gpui-base` for the 0.6.x component/base split.
- Disabled publishing for the vendored packages (`0.7.0-studio.1`).

Rules:

- Keep protocol mapping in Studio crates, never in upstream component source.
- Keep X11 disabled until the separately-audited opt-in backend exists.
- Record all source changes to the vendor tree here and in `UPSTREAM.md`.
- Update GPUI and gpui-kit together behind renderer and no-X11 checks.
