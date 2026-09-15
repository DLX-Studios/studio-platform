# Tasks: Studio Script Developer Experience

**Input**: Design documents from `specs/008-studio-devex/`

**Tests**: Required by the Studio constitution; every behavioral task follows red-green-refactor.

## Phase 1: Setup

- [X] T001 Vendor `rsvelte_fmt`, `rsvelte_formatter`, `tailwind_class_order`, `rsvelte_lint`, `rsvelte_diagnostics`, `rsvelte_preprocess`, `rsvelte_projection` at pinned rev with manifest-only lint deltas; record in `vendor/rsvelte/UPSTREAM.md`.
- [X] T002 [P] Scaffold `editors/vscode/` (`package.json`, `src/extension.ts` skeleton, `syntaxes/studio.tmLanguage.json` skeleton).

## Phase 2: Foundational

- [X] T003 Wire the three crates into `crates/studio-language-server/Cargo.toml` (`default-features = false`) and verify `cargo check -p studio-language-server` builds.
- [X] T004 Implement `format.rs` (FormatSession composition, `.studio` mapping, idempotence) with golden tests in `crates/studio-language-server/tests/format_golden.rs`.

## Phase 3: User Story 1 — Format, Lint, Navigate (P1)

**Independent test**: stdio round-trips for format/diagnostics/definition/rename/references against fixtures match goldens.

- [X] T005 [US1] Implement `lint.rs` (upstream diagnostics + Studio rules layer, ordered stream) with fixture tests in `crates/studio-language-server/tests/lint_fixtures.rs`.
- [X] T006 [US1] Implement `projection.rs` (mapping-backed definition/rename/references with gap reporting) with tests in `crates/studio-language-server/tests/projection_maps.rs`.
- [X] T007 [US1] Extend `intelligence.rs` (catalog/SDK/rune/route/asset completion + hover) with catalog fixture tests.
- [X] T008 [US1] Verify US1 acceptance with the four suites.

## Phase 4: User Story 2 — Thin Client (P2)

**Independent test**: Packaged extension activates, highlights, commands, and degrades per acceptance (manual verification).

- [X] T009 [P] [US2] Implement the extension client, grammar, and five commands in `editors/vscode/`.

## Phase 5: User Story 3 — Tooling Never Touches the Compiler (P1)

**Independent test**: Injected upstream failures change nothing about compiler/IR/build behavior.

- [X] T010 [US3] Add failure-isolation tests in `crates/studio-language-server/tests/isolation.rs` (malformed inputs, formatter rejection, projection gaps, version-skew fixtures).
- [X] T011 [US3] Verify US3 acceptance with the suite plus a `studio build` parity check on the same sources.

## Phase 6: Polish and Cross-Cutting Validation

- [ ] T012 [P] Update `docs/ROADMAP.md` feature 008 status and record evidence in `specs/008-studio-devex/validation-report.md`.
- [ ] T013 Full gate deferred to the post-008 verification phase.

## Dependencies and execution order

T001–T004 foundation first (vendoring, wiring, formatting). US1 builds lint, projection,
and intelligence on it. US2 (client) and US3 (isolation) proceed in parallel once the
server works.

## MVP scope

T001–T008: vendored crates, formatted/linted/navigable server proven by goldens and
protocol tests.
