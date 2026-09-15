# Tasks: Asset Imports and Lucide Icons

**Input**: Design documents from `specs/007-asset-imports/`

**Tests**: Required by the Studio constitution; every behavioral task follows red-green-refactor.

## Phase 1: Setup

- [X] T001 Add `lucide-static@1.40.0` to `package.json` and install it (`bun.lock` update), and add `oxc_parser` + `oxc_span` (0.146 line) to `crates/studio-cli/Cargo.toml`.
- [X] T002 [P] Create the fixture project `crates/studio-cli/tests/fixtures/icons/` (referenced, commented, renamed, missing, override, manifest-declared cases).

## Phase 2: Foundational

- [X] T003 Implement the typed reference graph and collector in `crates/studio-cli/src/assets.rs`: oxc-parsed call shapes with spans, package resolution, override precedence, sorted effective set, byte budget, staged writes.

## Phase 3: User Story 1 — Referenced Icons Only (P1)

**Independent test**: Fixture collects exactly the referenced set; missing fails with spans.

- [X] T004 [US1] Add failing matrix tests in `crates/studio-cli/tests/asset_icons.rs` (referenced/renamed/comment/missing cases).
- [X] T005 [US1] Verify US1 acceptance with the matrix suite.

## Phase 4: User Story 2 — Determinism, Manifests, Overrides (P1)

**Independent test**: Repeated builds byte-identical with untouched manifest; overrides win.

- [X] T006 [P] [US2] Add determinism/override/manifest-untouched tests in `crates/studio-cli/tests/asset_icons.rs`.
- [X] T007 [US2] Switch `collect_lucide` in `crates/studio-cli/src/build/mod.rs` to the typed collector with staged output and no manifest writes; feed the effective set to packaging.
- [X] T008 [US2] Verify US2 acceptance with the suite.

## Phase 5: User Story 3 — License and Budget (P2)

**Independent test**: Provenance entry exists; over-budget collection fails.

- [X] T009 [P] [US3] Add budget tests in `crates/studio-cli/tests/asset_icons.rs` and record version/license in `THIRD_PARTY_NOTICES.md`.
- [ ] T010 [US3] Verify US3 acceptance with the suite.

## Phase 6: Polish and Cross-Cutting Validation

- [ ] T011 [P] Update `docs/ROADMAP.md` feature 007 status and record evidence in `specs/007-asset-imports/validation-report.md`.
- [ ] T012 Full gate deferred to the post-008 verification phase.

## Dependencies and execution order

T001–T003 foundation first (package, parser dep, collector). US1 proves the graph; US2
rewires the pipeline onto it; US3 records provenance and budget.

## MVP scope

T001–T005 plus T007: typed collection proven by the matrix, wired into the real pipeline.
