# ADR-0004: Published GPUI Kit and native platform backends

- Status: Accepted by project owner request
- Date: 2026-10-31
- Supersedes: ADR-0003 and the original Wayland-only platform restriction

Studio and neighboring GPUI applications align on published GPUI Kit 0.8.1. Kit selects
the matching GPUI 0.3.10 family, including native platform dependencies and macOS font support.
Studio removes its old dependency-only vendor fork and consumes the Kit facade directly.
Linux builds include both Wayland and X11; macOS and Windows use upstream native backends.

The Runtime and Designer share a display preflight. On Linux either Wayland endpoint form
or DISPLAY permits startup; GPUI opens the actual connection. Other native platforms do not
require Linux environment variables. A declared endpoint is not proof of a usable connection.

CI verifies the native platform feature graph and retains the headless Wayland smoke test.
No-X11 feature/linkage gates are retired. Guest capabilities, retained UI semantics, signing,
and host authority are unchanged. Display-loss cleanup remains terminal.

This change enables upstream platform backends; it does not certify every Studio subsystem
on every OS. Native builds, OS credential integration, reload IPC, and runtime smoke tests
must be verified on each target before claiming release support. The reload transport gained a Windows AF_UNIX adapter on 2026-11-01; see
[reload platform adapter](../development/RELOAD_PLATFORM_ADAPTER.md) for scoped verification.
Web/mobile application
bootstraps and packaging are separate from aligning the native dependency family.

Published release references: https://gpui-kit.com/docs/getting-started/ and
https://crates.io/crates/gpui-kit/0.8.1 . Historical milestone specs that require Wayland-only
behavior are superseded on that point by this decision and constitution 2.0.0.
