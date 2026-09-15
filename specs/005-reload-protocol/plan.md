# Implementation Plan: Runtime Reload and Preview Protocol

**Branch**: `005-reload-protocol` | **Date**: 2026-09-03 | **Spec**: [spec.md](spec.md)

## Summary

Add a local reload channel between `studio dev` and the runtime: Unix domain socket at
`<project>/build/.studio-reload.sock` (passed via `--reload-socket`), newline-delimited JSON
`ReloadRequest`/`ReloadOutcome` protocol owned by a new `studio-host::reload` module (host
services shared by first-party processes). The runtime prepares (reuses `studio-package`
verification + instantiation without mounting), then commits (window-preserving surface swap +
disposal) or rolls back. The CLI replaces the refresh placeholder with request/response handling
plus a `restart-session` command. Route preserved-if-declared-else-default; instance state
always fresh. E2E runs against a stub host over loopback.

## Technical Context

**Language/Version**: Rust nightly-2026-03-04 (pinned)

**Primary Dependencies**: std `os::unix::net` (no new crates), `studio-package` verification,
`studio-host` services, `serde_json` newline-delimited messages

**Storage**: N/A; socket file in the project build dir, removed on clean exit

**Testing**: cargo integration tests with stub host over loopback sockets ( e2e edit→rebuild→
reload scenarios); display-level reload stays on the manual/headless gate pattern

**Target Platform**: Linux desktop/POS on native Wayland

**Project Type**: CLI toolchain + runtime host IPC inside the native desktop workspace

**Performance Goals**: Swap completes within the existing interaction budget; reload requests
never block the watch loop (single-flight with supersede, timeout-bounded await)

**Constraints**: Local IPC only (no network); dev trust semantics unchanged; production
admission untouched; atomic prepare/commit; stale-socket tolerant

**Scale/Scope**: One dev session per project; one runtime child; JSON messages under existing
bounded limits

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

PASS. Host authority preserved (runtime validates before touching live state; CLI never
drives rendering). Test-first honored (scripted sessions, storm, recovery, e2e). Contracts
versioned (protocol version field; additive outcomes). No new network/storage/capability
paths; Unix socket is local IPC. Small traceable slices per story.

## Project Structure

```text
specs/005-reload-protocol/
├── plan.md, research.md, data-model.md, quickstart.md, contracts/, tasks.md

crates/studio-host/src/reload.rs      # protocol types, session revision, swap machine (new)
crates/studio-cli/src/watch.rs        # client: send/await/supersede, restart-session (extend)
crates/studio-cli/src/main.rs         # restart-session subcommand (extend)
crates/studio-app/src/reload.rs       # server: listener, prepare/commit/rollback wiring (new)
crates/studio-host/tests/reload_session.rs      # scripted sessions (new)
crates/studio-host/tests/reload_storm.rs        # supersede + recovery (new)
tests/e2e-ish: crates/studio-cli/tests/reload_e2e.rs  # edit→rebuild→reload vs stub host (new)
```

**Structure Decision**: Protocol + state machine live in `studio-host` (shared first-party
services); thin client in `studio-cli`, thin server wiring in `studio-app`. No new crates.

## Complexity Tracking

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|-------------------------------------|
| None | N/A | Fits existing workspace boundaries; std Unix sockets, no new deps. |
