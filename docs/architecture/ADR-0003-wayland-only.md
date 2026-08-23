# ADR-0003: Wayland-Only Linux Host

- Status: Accepted
- Date: 2026-08-03

## Decision

The initial Studio host supports native Wayland only. X11 and XWayland are neither compiled as fallbacks nor accepted as runtime substitutes.

## Enforcement

- Pin `nightly-2026-03-04`; the selected GPUI revision currently uses unstable Rust APIs.
- Build GPUI and component dependencies without X11 features.
- Build CI with `DISPLAY` unset.
- Inspect release linkage for X11/XCB libraries.
- Exercise the host under a headless Wayland compositor.
- Return a controlled diagnostic when no Wayland connection is possible.
