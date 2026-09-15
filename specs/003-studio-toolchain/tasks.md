# Tasks: Studio Toolchain and Development Workflow

**Input**: Design documents from `specs/003-studio-toolchain/`

**Tests**: Required by the Studio constitution; every behavioral task follows red-green-refactor.

## Phase 1: Setup

- [X] T001 Split `crates/studio-cli/src/main.rs` into modules (`build/mod.rs`, `watch.rs`, keeping existing behavior identical) with `cargo test --locked -p studio-cli` and `cargo clippy --locked -p studio-cli --all-targets -- -D warnings` green.
- [X] T002 [P] Add a minimal fixture project under `crates/studio-cli/tests/fixtures/` (assembly source with a valid module plus a deliberately broken variant) and register the `[[test]]` targets `build_pipeline`, `watch_session`, and `shim_parity` in `crates/studio-cli/Cargo.toml`.

## Phase 2: Foundational

- [X] T003 Add failing tests asserting the documented exit-code table and diagnostic record shape (phase/code/severity/message/line/column JSON on stderr) in `crates/studio-cli/tests/build_pipeline.rs`.
- [X] T004 Implement the closed diagnostic registry and exit-code mapping in `crates/studio-cli/src/build/diagnostics.rs` (namespaced codes; packaging reuses studio-package error families verbatim; replace the ad-hoc `STUDIO_IO` literal) per `specs/003-studio-toolchain/contracts/cli.md`.
- [X] T005 Make generators content-stable in `crates/studio-cli/src/`: `generate_routes` writes `examples/<name>/assembly/routes.generated.ts` and `collect_lucide` writes `assets/icons/*` only when rendered content differs from disk; add focused tests for the no-rewrite property.

## Phase 3: User Story 1 — One-Command Reproducible Builds (P1)

**Independent test**: Build the starter example from a clean state and verify the bundle; corrupt a source and verify a structured failure with no partial bundle; rebuild identical inputs and verify byte-identical artifacts.

- [X] T006 [P] [US1] Add failing integration tests in `crates/studio-cli/tests/build_pipeline.rs`: clean build produces `examples/starter/build/starter.studio` with exit 0; compile failure exits 11 with `phase:"compile"` diagnostics and leaves the destination untouched; repeated identical builds are byte-identical.
- [X] T007 [US1] Implement the phase pipeline in `crates/studio-cli/src/build/mod.rs`: validate → compile (lucide, routes, asc) → package via `studio_package::pack_bundle`, with per-phase diagnostics threaded through `build/diagnostics.rs`; stop swallowing lucide/routes errors.
- [X] T008 [US1] Implement atomic bundle output in `crates/studio-cli/src/build/mod.rs` (temp file in the destination directory + rename; failures never touch the destination) and switch `main()` to the documented `ExitCode` mapping.
- [X] T009 [US1] Verify US1 acceptance with `cargo test --locked -p studio-cli --test build_pipeline`, a manual `studio build starter` run, and the byte-identity `cmp` check from `specs/003-studio-toolchain/quickstart.md`.

## Phase 4: User Story 2 — Trustworthy Development Watch Loop (P1)

**Independent test**: Drive the watch harness with generated-file churn, five rapid saves, a mid-build change, and a cancellation; verify zero self-triggers, coalesced builds, single-flight supersede, and clean shutdown.

- [X] T010 [P] [US2] Add failing watch tests in `crates/studio-cli/tests/watch_session.rs` using a library-level harness with an injectable clock/sink: generated churn (`routes.generated.ts`, lucide icon rewrites) triggers nothing; five rapid saves coalesce into one build; a change during a build yields exactly one trailing rebuild; cancellation stops cleanly with no partial output.
- [X] T011 [US2] Implement the content-hash watcher in `crates/studio-cli/src/watch.rs`: hash-based change detection over `assembly/`, `routes/`, `assets/` excluding generator outputs, 300 ms settle window, single-flight with supersede, and clean Ctrl-C cancellation exiting 130.
- [X] T012 [US2] Surface runtime launch failures as structured diagnostics in `crates/studio-cli/src/watch.rs` (replace the swallowed spawn result; `BUILD_WATCH_RUNTIME` diagnostic, watch failure exit family) and keep `studio dev`/`studio preview` behavior aligned per `specs/003-studio-toolchain/research.md` R5.
- [X] T013 [US2] Verify US2 acceptance with `cargo test --locked -p studio-cli --test watch_session` and the manual watch scenarios from `specs/003-studio-toolchain/quickstart.md` step 4.

## Phase 5: User Story 3 — Toolchain-Owned Packaging (P2)

**Independent test**: Run each documented TS entry and the native command on the same inputs and verify identical bundles and exit codes.

- [X] T014 [P] [US3] Add failing parity tests in `crates/studio-cli/tests/shim_parity.rs`: `bun run ./scripts/build-example.ts starter` and `studio build starter` produce byte-identical bundles and identical exit codes for the success and usage-failure cases.
- [X] T015 [US3] Convert `scripts/build-example.ts` into a delegating shim: ensure the `studio` binary is built using the shared-cache cargo pattern from `scripts/build-studio.ts`, spawn `studio build <example>`, propagate the exit code verbatim; keep `package.json` entries (`build:starter`, `build:pos`) unchanged.
- [X] T016 [US3] Verify US3 acceptance with `cargo test --locked -p studio-cli --test shim_parity`, `bun run build:starter`, `bun run build:pos`, and `bun test tests/e2e/starter_plugin.test.ts`.

## Phase 6: User Story 4 — Automation-Friendly Diagnostics (P2)

**Independent test**: Exercise each documented failure family and verify the exit code alone classifies it identically to the diagnostics.

- [X] T017 [P] [US4] Add a classification test in `crates/studio-cli/tests/build_pipeline.rs` asserting each failure family (usage, validate, compile, package, io) maps to its unique documented exit code with matching `phase` in diagnostics.
- [X] T018 [US4] Document the command surface, exit codes, and diagnostic shape in `docs/development/BUILDING.md` and `specs/003-studio-toolchain/contracts/cli.md` stays authoritative; verify `studio --help` reflects the contract.

## Phase 7: Polish and Cross-Cutting Validation

- [X] T019 [P] Update `docs/ROADMAP.md` feature 003 status and record the validation evidence in `specs/003-studio-toolchain/validation-report.md`.
- [X] T020 Run the full gate: `cargo fmt --all -- --check`, `cargo clippy --locked --workspace --all-targets -- -D warnings`, `cargo test --locked --workspace`, `bun run check`, `bun test`, and `./scripts/test-starter-quickstart.sh`; record results in the validation report.

## Dependencies and execution order

Foundational T003–T005 block all stories (diagnostics/exit codes and content-stable generation
are prerequisites for deterministic builds and churn-free watching). US1 is the MVP and enables
the pipeline the watcher and shim build on. US2 and US3 depend on US1's pipeline but are
independently testable; US4's classification test depends on the registry from Phase 2.

## MVP scope

T003–T009: the diagnostic/exit-code foundation plus the phased, atomic, deterministic build
pipeline proven by clean/failed/repeated integration tests.
