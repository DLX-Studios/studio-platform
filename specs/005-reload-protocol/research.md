# Research: Runtime Reload and Preview Protocol

**Date**: 2026-09-03 · Resolves plan-level unknowns for [spec.md](spec.md).

## R1. Transport and socket discipline

**Decision**: Unix domain `SOCK_STREAM` socket at `<project>/build/.studio-reload.sock`,
created by the runtime on `--reload-socket <path>`, removed on clean exit. Newline-delimited
JSON messages both ways. Stale-socket rule: the CLI attempts connect; on connection-refused
the file is stale and may be unlinked by the next session start; the runtime unlinks its own
socket on startup before binding. No abstract-namespace sockets (filesystem path keeps cleanup
observable and testable).

**Rationale**: Local-only by construction (no TCP, no network namespace issues); std-only, no
new crates; matches the repo's JSON-lines conventions (diagnostics, protocol fixtures).

**Alternatives considered**: TCP loopback (rejected: opens network surface and port-picking
complexity for zero benefit); file-drop signaling (rejected: no acknowledgements, racy).

## R2. Protocol shape

**Decision**: Versioned envelopes, additive-only:
`{"protocol":"studio-reload/1","type":"reload","request_id","bundle","base_revision"}` →
`{"protocol":"studio-reload/1","type":"reloaded","request_id","revision"}` or
`{"type":"rejected","request_id","stage","code","message"}`; plus
`{"type":"restart","request_id"}` → `{"type":"restarted","request_id","revision"}`.
`stage` is one of `validate|instantiate|commit`. Timeouts: CLI awaits at most 30 s, then
treats the session as lost (structured watch diagnostic, existing 003 behavior).

**Rationale**: Request IDs correlate across the single-flight supersede; stage naming reuses
the pipeline vocabulary authors already see.

## R3. Prepare/commit split in the runtime

**Decision**: Prepare reuses `studio-package` verification plus the existing
`prepare_internal` admission path up to instantiation, without mounting; commit swaps the
`PluginSurface` inside the live window and disposes the old instance; rollback drops the
prepared instance and keeps the live one. Disposal is cancellable until commit returns.

**Rationale**: Reuses the exact admission the host already trusts; the two-phase shape is what
makes "never a dead window" structural rather than aspirational.

## R4. State policy mechanics

**Decision**: On commit, the runtime reads the new bundle's declared route set: current route
preserved when still declared, else the default route. Plugin instance state is always fresh
(new instantiation by construction). Navigation state handle comes from the existing route
ownership (001/002), not the file-based router (006).

## R5. Test strategy without a display

**Decision**: Automated e2e runs a stub runtime host (implements the swap machine over a
loopback socket) against the real CLI client path: scripted 10-reload scenario, storm
supersede, crash/validation failures, restart, cancel. Display-level reload (real window swap
under a compositor) stays on the manual/headless gate pattern with a documented scenario.
