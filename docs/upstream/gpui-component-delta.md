# gpui-component Fork Policy

Upstream revision: `fb26e617da3add2ce2ac92a2ccc1a64bc8343135` (package `0.6.1`).

Audit completed: 2026-08-04 for `e1570bdc8fd2dc17d38cab09e74b1783bdf3b24b` (package `0.5.2`);
synchronized 2026-09-17 to `fb26e61` (package `0.6.1`). This revision replaces the former
single-crate vendor tree with the upstream `component`/`base` split plus the required
`assets` and `macros` crates.

## Retained scope

Studio uses upstream components as native controls: Button is integrated in the initial renderer,
and Input, Select, and Slider are available to the runtime for protocol mapping. Layout containers
remain host-owned GPUI layout primitives because they are the native compositional API underneath
the component library.

The full source is retained rather than copied piecemeal, so component behavior, accessibility,
and visual semantics stay upstream-compatible. It also avoids maintaining local imitations of
Button, Input, Select, or Slider.

## Enabled features

- GPUI: `gpui-pre 0.3.5` (Zed snapshot `d89e9c2`) with `default-features = false`, `wayland` only.
- gpui-component: no default optional features.
- No X11/XWayland feature is enabled in the shipping dependency graph.

The project can add an opt-in X11 backend later; it must be a separate build feature and receive
its own dependency and release-binary audit. The present runtime remains Wayland-only.

## Local deltas

- Replaced upstream workspace dependencies with explicit versions and local asset/macro paths.
- Moved GPUI off the Zed git pin onto crates.io `gpui-pre 0.3.5` (same Zed code, registry
  distribution); `gpui-pre-macros`/`gpui-pre-sum-tree` follow at `0.3.5`.
- Added Wayland-only GPUI dependency declarations in all four vendored crates
  (`gpui-component`, `gpui-base`, `gpui-kit-assets`, `gpui-component-macros` where applicable).
- Adopted the upstream `gpui-kit-assets` package name (`vendor/gpui-kit-assets`) and added
  `vendor/gpui-base` for the 0.6.x component/base split.
- Disabled publishing for the vendored packages (`0.6.1-studio.1`).

Rules:

- Keep protocol mapping in Studio crates, never in upstream component source.
- Keep X11 disabled until the separately-audited opt-in backend exists.
- Record all source changes to the vendor tree here and in `UPSTREAM.md`.
- Update GPUI and gpui-component together behind renderer and no-X11 checks.
