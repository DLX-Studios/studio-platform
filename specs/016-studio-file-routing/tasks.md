# Tasks: Studio File Routing

## Phase 1: Contracts

- [X] T001 Record requirements, architecture, closed routing contract and acceptance in specs/016-studio-file-routing/{spec,plan,research,data-model,quickstart}.md and contracts/routing.md (FR-001–008).

## Phase 2: Shared router and discovery — US1

- [X] T002 [P] Add failing root/wildcard/precedence/negative tests in crates/studio-navigation/tests/route_tree.rs and .studio/+page/layout-exclusion/duplicate tests in crates/studio-cli/tests/route_registry.rs (FR-001/002/004); root agent runs focused Cargo commands before implementation.
- [X] T003 [P] Extend crates/studio-navigation/src/{route,tree}.rs and crates/studio-cli/src/routes.rs; replace the independent CLI concrete matcher with shared route resolution while retaining legacy rendering (FR-001/002/004/007).

## Phase 3: Component and layout compilation — US2

- [X] T004 [P] Add failing component/props/scoped-state/layout/cycle/escape tests in crates/studio-script/tests/studio_project.rs (FR-002/003/006).
- [X] T005 [P] Implement bounded project composition in crates/studio-script/src/project.rs and contextual imported-name validation in validate.rs/lib.rs, preserving source diagnostics (FR-002/003/006/007).

## Phase 4: Host navigation and both compiler paths — US3

- [X] T006 Add failing dev and compiled guest navigation tests in crates/studio-cli/src/dev/surface.rs and crates/studio-script/tests/studio_project.rs (FR-004/005/006).
- [X] T007 Wire project discovery/check/build/dev and signed route metadata through crates/studio-cli/src/studio_project.rs, build/mod.rs, dev/{compiler,surface}.rs and main.rs (FR-001–007).
- [X] T008 Emit existing navigation commands and multi-page Wasm dispatch in crates/studio-script/src/assemblyscript.rs, and validate host routing/remounts in crates/studio-runtime/src/plugin_surface.rs using studio-navigation (FR-004/005).

## Phase 5: Example, documentation and verification — US4

- [X] T009 Replace examples/stream-countdown/app.studio with routed pages/layout/components; document commands, navigation and supported native subset (FR-008).
- [X] T010 Run focused regressions then sequential format/workspace-test/Clippy integration gates; record actual results in validation-report.md and reconcile scope through convergence (FR-001–008).

- [X] T011 Reconcile SDK root navigation, malformed-path rejection and positional route precedence in sdk/assemblyscript/assembly/navigation.ts with red-green SDK matching tests (FR-004/005/007).

## Phase 6: Automatic stream countdown

- [X] T012 Add red-green bounded timer metadata/scheduler tests and implementation in crates/studio-navigation/src/timers.rs and tests/timers.rs (FR-009).
- [X] T013 Normalize source-only Timer declarations into invisible native event/status nodes in crates/studio-script/src/project.rs; collect scoped timer metadata and test negative declarations and imported instances (FR-003/009).
- [X] T014 Admit signed timer metadata and deliver due events through existing Wasm/IR handlers in crates/studio-runtime/src/{host,plugin_surface}.rs and crates/studio-cli/src/dev/surface.rs; validate manual-clock catch-up, completion and route cancellation in both paths (FR-005/009).
- [X] T015 Update the stream example to automatic 600-second countdown, opacity exit transition and delayed live navigation; document and verify (FR-008/009).

- [X] T016 Add red-green closed native presentation property/renderer tests and implement viewport-cover Box background assets, bounded Text size/color, and retained opacity interpolation; finish GTA-inspired static header artwork in the example (FR-010).

## Dependencies and implementation

T001 precedes work. T002/T004 are independent first-failure tests; T003 and T005 follow their red evidence and may proceed in parallel in separate files. T006 follows both foundations. T007/T008 form one cross-path integration slice. T009 then T010 close the feature. Only the root agent runs Cargo, using the shared serialized target configuration.

## Follow-up tracked scope

Automatic countdown scheduling and the native GTA-inspired presentation are included in FR-009/010 and T012–T016. Designer tickets 44/68/69 retain their broader acceptance scope; this feature does not close their unrelated Designer journeys.
