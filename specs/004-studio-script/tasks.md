# Tasks: Studio Script and Embedded Development Host

**Input**: Design documents from `specs/004-studio-script/`

**Tests**: Required by the Studio constitution; every behavioral task follows red-green-refactor.

## Phase 1: Setup

- [X] T001 Vendor the `rsvelte` facade, `rsvelte_core` parse API, `rsvelte_esrap`, `rsvelte_ast_equiv`, and `rsvelte_projection` manifests under `vendor/rsvelte/` at pinned rev `b64aed9c` (git checkout fails on an SSH submodule URL, so vendoring follows the `vendor/gpui-component` precedent) with manifest-only lint deltas recorded in `vendor/rsvelte/UPSTREAM.md`; depend on them from `crates/studio-script/Cargo.toml` with `default-features = false` and no `projection`; `cargo check -p studio-script` builds the new dependency closure.
- [X] T002 [P] Create the versioned IR fixture layout `crates/studio-script/tests/fixtures/ir-v2/<name>/{source.studio,module.json,protocol.jsonl}` with the six reference fixtures (props-defaults, state-derived, conditional, keyed-each, interpolation, typed-events).

## Phase 2: Foundational

- [X] T003 Implement the fenced frontend adapter in `crates/studio-script/src/rsvelte_adapter.rs`: `Engine::prepare` wrapper, fingerprint surfacing, facts extraction, and error-to-`Diagnostic` mapping; no `rsvelte` type crosses the module boundary, and `compile()`/`compile_both()` are never called.
- [X] T004 Implement the closed `StudioType` system in `crates/studio-script/src/types.rs` (one canonical `number`; integral-i64-range → Integer, other numbers → Decimal host mapping; arrays, maps, records, SDK types) with unit tests for the numeric rule.
- [X] T005 [P] Extend the IR types to the v2 dynamic model in `crates/studio-script/src/ir.rs` (components, typed props, state/derived slots, handlers, If/Each/Interpolation nodes, typed expressions) with serde round-trip tests; bump `STUDIO_IR_VERSION` to 2 and keep the v1 lowering path green.

## Phase 3: User Story 1 — Author Once, Run Natively (P1)

**Independent test**: The six reference fixtures compile with zero diagnostics; serialized IR matches `module.json` goldens; invalid modules fail with spanned `STUDIO3xx` codes and no IR.

- [X] T006 [P] [US1] Add failing subset-rejection tests in `crates/studio-script/tests/studio_validate.rs` covering every rejected-construct family per `specs/004-studio-script/contracts/studio-diagnostics.md`.
- [X] T007 [US1] Implement the portable-subset validator in `crates/studio-script/src/validate.rs` (runes, types, approved calls, closed catalog kinds, routes/assets) with additive `STUDIO3xx` codes.
- [X] T008 [P] [US1] Add failing IR golden tests in `crates/studio-script/tests/studio_ir_v2.rs` asserting byte-identical `module.json` for all six fixtures.
- [X] T009 [US1] Implement AST→IR lowering from the adapter facts through the validator in `crates/studio-script/src/lower_studio.rs` (stable IDs, typed slots, template/expression trees).
- [X] T010 [US1] Verify US1 acceptance: reference set compiles clean with golden-matching protocol output; every rejected family has a code and span; no IR on invalid input.

## Phase 4: User Story 2 — Identity-Preserving Hot Swap (P1)

**Independent test**: A 10-edit scripted session over a stateful component shows preserved state, retained identity, rejection-and-continue on invalid edits, dispose-and-initialize on type changes, and ordered dependent reevaluation.

- [X] T011 [P] [US2] Add failing evaluator tests in `crates/studio-script/tests/studio_eval.rs`: mount projections match `MountTree` goldens; dispatches produce the expected protocol messages over a retained registry fixture.
- [X] T012 [US2] Implement the Rust expression evaluator and template projection in `crates/studio-script/src/eval.rs` over `UiRegistry::mount/apply_patch` and `HostEventDispatcher` semantics.
- [X] T013 [P] [US2] Add failing hot-swap tests in `crates/studio-script/tests/studio_hot_swap.rs`: transaction outcomes (accepted with migration report / rejected with last-valid running), compatible-slot preservation, genuine type-change reset, rollback on host failure, dependent ordering.
- [X] T014 [US2] Implement `replace_module_atomically` in `crates/studio-script/src/hot_swap.rs` per the swap contract in `docs/STUDIO_SCRIPT_TRANSFORM_PIPELINE.md` §8.
- [X] T015 [US2] Verify US2 acceptance with the three suites above plus the swap-budget check from `specs/004-studio-script/quickstart.md` step 3.

## Phase 5: User Story 3 — Production Wasm from the Same IR (P1)

**Independent test**: Shared fixtures emit identical observable protocol sequences through the Rust evaluator, the oracle, and compiled Wasm; ASC failures are structured.

- [X] T016 [P] [US3] Add failing lowering tests in `crates/studio-script/tests/studio_emit.rs`: generated assembly is deterministic and imports the SDK factories/bootstrap for the reference fixtures.
- [X] T017 [US3] Extend the AssemblyScript backend in `crates/studio-script/src/lower/assemblyscript.rs` (SDK-targeting component modules, ASC-only bootstrap, reserved virtual modules, widened `NODE_KIND_*` constants).
- [X] T018 [US3] Add failing differential tests in `crates/studio-script/tests/studio_differential.rs` asserting byte-identical `protocol.jsonl` across evaluator, oracle, and Wasm legs.
- [X] T019 [US3] Wire the differential harness (evaluator + `simulate_event` + starter-style Wasm harness) in `crates/studio-script/tests/studio_differential.rs`.
- [X] T020 [US3] Verify US3 acceptance: Wasm admitted by host trust machinery; full fixture equivalence; lowering failures structured.

## Phase 6: User Story 4 — Deterministic Build Lifecycle (P2)

**Independent test**: Repeated builds byte-identical; edits invalidate only the affected chain; dev edits never trigger ASC until explicit build/session boundary.

- [X] T021 [P] [US4] Add failing module-graph tests in `crates/studio-script/tests/studio_graph.rs`: static import resolution, cycle naming, dependency-ordered invalidation.
- [X] T022 [US4] Implement the module graph in `crates/studio-script/src/graph.rs`.
- [X] T023 [P] [US4] Add failing fingerprint tests in `crates/studio-script/tests/studio_fingerprint.rs`: identical inputs → identical keys; any toolchain input drift changes keys.
- [X] T024 [US4] Implement `ArtifactFingerprint` in `crates/studio-script/src/fingerprint.rs` and wire the `.studio` compile path into `studio check`/`build`/`dev` per `specs/004-studio-script/contracts/studio-cli-surface.md`.
- [X] T025 [US4] Verify US4 acceptance with `cargo test -p studio-script` plus the offline and determinism scenarios in `specs/004-studio-toolchain/`-style quickstart steps 5–6.

## Phase 7: Polish and Cross-Cutting Validation

- [X] T026 [P] Update `docs/ROADMAP.md` feature 004 status and record evidence in `specs/004-studio-script/validation-report.md`.
- [X] T027 Run the full gate: `cargo fmt --all -- --check`, `cargo clippy --locked --workspace --all-targets -- -D warnings`, `cargo test --locked --workspace`, `bun run check`, `bun test`, and the feature quickstart; record results.

## Dependencies and execution order

Setup T001–T002 unblock everything (pinned frontend + fixture layout). Foundational T003–T005
(adapter, types, IR v2) block all stories. US1 is the MVP and enables the validated-IR contract
the evaluator and backends consume. US2 builds on US1's IR plus the module graph query surface
(independently testable with in-test graphs). US3 consumes US1 IR and the evaluator vocabulary
from US2. US4 composes graph + fingerprints + CLI wiring. Polish follows all batches.

## MVP scope

T001–T010: pinned frontend, types, IR v2, validator, AST→IR lowering, and golden fixtures —
the compile path with user-visible validation, before any evaluation or lowering work.
