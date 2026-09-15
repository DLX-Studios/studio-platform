# Tasks: Runtime Reload and Preview Protocol

**Input**: Design documents from `specs/005-reload-protocol/`

**Tests**: Required by the Studio constitution; every behavioral task follows red-green-refactor.

## Phase 1: Setup

- [X] T001 Add the `studio-host::reload` module skeleton with protocol message types (`ReloadRequest`, `ReloadOutcome`, restart variants) and JSON round-trip tests in `crates/studio-host/src/reload.rs`.

## Phase 2: Foundational

- [X] T002 Implement the session revision counter and the swap state machine (`Preparing → Prepared/PrepareFailed → Committing → Committed/RolledBack`) with disposal-cancellable-until-commit semantics in `crates/studio-host/src/reload.rs`.
- [X] T003 Add failing scripted-session tests in `crates/studio-host/tests/reload_session.rs`: 10 scripted reloads (valid, invalid, crashing) with outcome assertions and revision accounting.

## Phase 3: User Story 1 — Edit, Rebuild, Reload (P1)

**Independent test**: Scripted session over a real loopback channel swaps on valid, rejects invalid with the surface intact, and names failing stages.

- [X] T004 [US1] Implement the Unix socket endpoint discipline (bind with own-stale-unlink, connect with refused-as-stale, remove on clean exit) plus timeout-bounded request/response in `crates/studio-host/src/reload.rs`.
- [X] T005 [US1] Wire the CLI client into `crates/studio-cli/src/watch.rs`: pass `--reload-socket` to the runtime child, send reload per successful build with single-flight supersede, map outcomes to diagnostics and exit codes, replacing the refresh placeholder.
- [X] T006 [US1] Wire the server side in `crates/studio-app/src/reload.rs`: listener thread, prepare via existing verification without mounting, commit with window-preserving swap, rollback path.
- [X] T007 [US1] Add failing e2e in `crates/studio-cli/tests/reload_e2e.rs`: real client path against a stub host — build, reload, reject with intact surface, stage naming.
- [X] T008 [US1] Verify US1 acceptance with the scripted, storm, and e2e suites.

## Phase 4: User Story 2 — Failed Reloads Never Kill the Window (P1)

**Independent test**: Init crashes, validation failures, and mid-swap errors all end on a live surface with diagnostics.

- [X] T009 [P] [US2] Add failing recovery tests in `crates/studio-host/tests/reload_storm.rs`: crash-on-init keeps live, mid-swap failure cancels disposal, first-bundle failure yields safe empty state.
- [X] T010 [US2] Implement rollback and safe-empty-state paths in `crates/studio-host/src/reload.rs` and the `studio-app` wiring from T006.

## Phase 5: User Story 3 — Session Control (P2)

**Independent test**: Restart disposes-then-instantiates with exact-once revision; cancel exits 130 with socket removed.

- [X] T011 [P] [US3] Add the `restart-session` subcommand to `crates/studio-cli/src/main.rs` resolving the project socket and issuing the restart request.
- [X] T012 [US3] Implement restart handling (dispose-then-instantiate, exact-once revision) and cancellation cleanup (child kill, socket removal, exit 130) across `crates/studio-cli/src/watch.rs` and `crates/studio-app/src/reload.rs`.

## Phase 6: User Story 4 — Preserved/Reset State (P2)

**Independent test**: Reload across route sets preserves declared routes, resets to default otherwise, with fresh instances.

- [X] T013 [P] [US4] Add failing state-policy tests in `crates/studio-host/tests/reload_session.rs`: preserved route, removed route resets, fresh instance evidence.
- [X] T014 [US4] Implement route resolution on commit (preserved-if-declared-else-default) in `crates/studio-app/src/reload.rs` using the existing route ownership.

## Phase 7: Polish and Cross-Cutting Validation

- [X] T015 [P] Update `docs/ROADMAP.md` feature 005 status and record evidence in `specs/005-reload-protocol/validation-report.md`.
- [ ] T016 Run the full gate: `cargo fmt --all -- --check`, `cargo clippy --locked --workspace --all-targets -- -D warnings`, `cargo test --locked --workspace`, `bun run check`, `bun test`; record results.

## Dependencies and execution order

Setup and foundational (T001–T003) block all stories. US1 is the MVP (channel + swap
wiring + e2e). US2 builds on the swap machine with recovery paths. US3 and US4 are
independently testable once US1's channel exists.

## MVP scope

T001–T008: protocol, swap machine, socket discipline, CLI/server wiring, and the e2e
proving edit→rebuild→reload.
