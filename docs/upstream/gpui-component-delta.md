# gpui-component Fork Policy

Upstream revision: `e1570bdc8fd2dc17d38cab09e74b1783bdf3b24b`.

Audit completed: 2026-08-04. This revision replaces the former animation-only fork with the
complete `gpui-component` UI package and its required `assets` and `macros` crates.

## Retained scope

Studio uses upstream components as native controls: Button is integrated in the initial renderer,
and Input, Select, and Slider are available to the runtime for protocol mapping. Layout containers
remain host-owned GPUI layout primitives because they are the native compositional API underneath
the component library.

The full source is retained rather than copied piecemeal, so component behavior, accessibility,
and visual semantics stay upstream-compatible. It also avoids maintaining local imitations of
Button, Input, Select, or Slider.

## Enabled features

- GPUI: `default-features = false`, `wayland` only.
- gpui-component: no default optional features.
- No X11/XWayland feature is enabled in the shipping dependency graph.

The project can add an opt-in X11 backend later; it must be a separate build feature and receive
its own dependency and release-binary audit. The present runtime remains Wayland-only.

## Local deltas

- Replaced upstream workspace dependencies with explicit versions and local asset/macro paths.
- Pinned GPUI to `381953d44897c53c4d252ae30620bafaa7d060b7` (current Zed main at the upgrade date).
- Added Wayland-only GPUI dependency declarations in all three vendored crates.
- Disabled publishing for the vendored packages.

Rules:

- Keep protocol mapping in Studio crates, never in upstream component source.
- Keep X11 disabled until the separately-audited opt-in backend exists.
- Record all source changes to the vendor tree here and in `UPSTREAM.md`.
- Update GPUI and gpui-component together behind renderer and no-X11 checks.
