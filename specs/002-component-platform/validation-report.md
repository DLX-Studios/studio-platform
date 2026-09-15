# Component Platform Validation Report

**Validation date:** 2026-09-03
**Implementation result:** T001–T036 are checked and the component-platform implementation is
covered by the focused and workspace Rust suites. This report separates automated implementation
evidence from JavaScript, graphical, hardware, and release-owner gates.

## Automated evidence

| Gate | Result | Evidence |
| --- | --- | --- |
| Formatting | PASS | `cargo fmt --all -- --check` |
| Rust lint | PASS | `cargo clippy --locked --workspace --all-targets -- -D warnings` |
| Rust workspace | PASS | `cargo test --locked --workspace`; the explicitly deferred N100 benchmark remains ignored |
| Component catalog | PASS | protocol catalog, native mapping, controls, overlays, navigation, data, and compatibility tests |
| Designer surfaces | PASS | identity shell, dashboard, canvas, content, agent, MCP, plugin/template, settings, sync, and recovery tests |
| Flagship deterministic harness | PASS | `studio-flagship` report passed all deterministic gates and retained external blockers |
| JavaScript install | PASS | T037: `bun install --frozen-lockfile` with the pinned Bun 1.3.9 (`~/.cache/studio-bun/1.3.9/bun`); 1249 packages installed, `bun.lock` unchanged. The earlier rejection came from a local Bun `1.4.0-canary.1`, not the checkout filesystem |
| JavaScript checks | PASS | T037: `bun run check` exited 0 (tsc + `@studio/sdk` ASC check; known-benign AS235 warnings only). Root `bun test`: 38 pass / 1 fail across 5 files; SDK contract suites `sdk/assemblyscript` `bun test tests`: 15 pass / 0 fail, including `component-catalog.test.ts` 2 pass / 0 fail. `bun run generate:protocol` followed by `git diff --exit-code -- protocol sdk/assemblyscript/assembly/generated` reported no drift (SC-008) |

The single failing root test is `tests/platform/wayland_startup.test.ts`, which is not a
component-platform gate: it launches the headless Sway compositor and fails fast with
"sway is required" on this host. Sway requires a password-protected sudo install
(`sudo apt-get install --yes sway` per the Debian/Ubuntu build packages in
`docs/development/BUILDING.md`); native compositor launch and cleanup evidence for that
platform gate already exists in `specs/001-secure-plugin-runtime/validation-report.md`.

## Scope and limitations

The Rust tests prove the host-owned contracts and deterministic behavior. They do not prove a
physical printer, three-station deployment, live Stripe sandbox receipt, visual assistive
technology behavior, or release-owner approval. The flagship report also identifies production
seams that must be integrated or explicitly accepted before release: RBAC employee row scope,
center topology, workflow scheduling, application audit integration, signed updates, generic POS
certification, content collections/forms, agent conversation UX, and plugin/template installation
UX.

The checkout mount is ext4, but scripts were migrated onto it without preserving modes, so
tracked-executable scripts had lost their executable bit; the modes were restored from the git
index (`chmod +x` on every `100755` path) as part of T037. Use the pinned Bun 1.3.9 and the
filesystem-safe procedures documented in
[`docs/development/BUILDING.md`](../../docs/development/BUILDING.md).
