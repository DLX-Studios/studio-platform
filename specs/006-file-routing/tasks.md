# Tasks: File-Based Routing

**Input**: Design documents from `specs/006-file-routing/`

**Tests**: Required by the Studio constitution; every behavioral task follows red-green-refactor.

## Phase 1: Setup

- [X] T001 Create the fixture route tree (static, nested, param, wildcard, underscore-excluded, duplicate, ambiguous, malformed ×4, title/mismatch cases) under `crates/studio-cli/tests/fixtures/routes/` and register `route_registry` in `crates/studio-cli/Cargo.toml`.

## Phase 2: Foundational

- [X] T002 Implement `RouteEntry` scanning, validation (duplicates, ambiguity, malformed, mismatch), deterministic render, and the pure matcher in `crates/studio-cli/src/routes.rs`.
- [X] T003 Switch `generate_routes` in `crates/studio-cli/src/build/mod.rs` to the registry renderer, keeping `route_<name>` and `declaredRoutes` output byte-identical and appending the table.

## Phase 3: User Story 1 — Declare Routes as Files (P1)

**Independent test**: Fixture scan yields the golden registry; every error family names files and problems.

- [X] T004 [US1] Add failing matrix tests in `crates/studio-cli/tests/route_registry.rs`: every shape maps, underscore excluded, duplicates/ambiguity/malformed/mismatch rejected.
- [X] T005 [US1] Add the golden render test (BLESS workflow) and verify US1 acceptance.

## Phase 4: User Story 2 — Resolve Paths (P1)

**Independent test**: 40-path battery resolves identically in Rust, the guest SDK, and the reload policy.

- [X] T006 [P] [US2] Add the 40-path battery in `crates/studio-cli/tests/route_registry.rs` covering static/nested/param/wildcard/miss semantics.
- [X] T007 [US2] Add `RouteTable` type plus pure `matchRoute` to `sdk/assemblyscript/assembly/navigation.ts` with `sdk/assemblyscript/tests/navigation-routes.test.ts` running the same battery.
- [X] T008 [US2] Switch the reload preserved-route check to pattern matching in `crates/studio-app/src/reload.rs` with unit tests for parameterized preservation and reset.
- [X] T009 [US2] Verify US2 acceptance: batteries agree across all three implementations.

## Phase 5: User Story 3 — Metadata (P2)

**Independent test**: Titles from exports, defaults, and mismatch diagnostics.

- [X] T010 [P] [US3] Cover title extraction, defaults, and `route`-export mismatch in `crates/studio-cli/tests/route_registry.rs` (implemented via the T002 scanner).

## Phase 6: Polish and Cross-Cutting Validation

- [X] T011 [P] Update `docs/ROADMAP.md` feature 006 status and record evidence in `specs/006-file-routing/validation-report.md`.
- [ ] T012 Full gate deferred to the post-008 verification phase.

## Dependencies and execution order

T001–T003 foundation first. US1 and US2 build on it (matcher battery shared by SDK and
reload tasks). US3 rides the same scanner. Polish last.

## MVP scope

T001–T005 plus T008: registry with golden output and pattern-aware reload preservation.
