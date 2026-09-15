# Tasks: Secure Native Plugin Runtime

**Input**: Design documents from `specs/001-secure-plugin-runtime/`

**Prerequisites**: `plan.md`, `spec.md`, `research.md`, `data-model.md`, `contracts/`,
`quickstart.md`, and `.specify/memory/constitution.md`

**Tests**: Required. Every behavioral task follows Red-Green-Refactor. Test tasks explicitly
create the first expected failure; the following implementation task makes that focused test pass
and runs the affected suite.

**Organization**: Tasks are grouped by user story. Shared platform, contract, testkit, and sandbox
work is completed first because no story can be safely implemented without those boundaries.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel with other marked tasks in the same group because it changes
  different files and does not depend on unfinished work.
- **[Story]**: Maps the task to a user story from `spec.md`.
- **[FR/SC]**: Records primary requirement and success-criterion traceability.
- Every task names exact files and its focused verification command.

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Expand the existing Phase 0 scaffold along the approved crate boundaries without
changing runtime behavior.

- [X] T001 Add `studio-package` and `studio-testkit` workspace members with minimal documented library roots in `Cargo.toml`, `crates/studio-package/Cargo.toml`, `crates/studio-package/src/lib.rs`, `crates/studio-testkit/Cargo.toml`, and `crates/studio-testkit/src/lib.rs`; verify with `cargo check --workspace`
- [X] T002 Add `studio-wasm` and `studio-ui` workspace members with minimal documented library roots in `Cargo.toml`, `crates/studio-wasm/Cargo.toml`, `crates/studio-wasm/src/lib.rs`, `crates/studio-ui/Cargo.toml`, and `crates/studio-ui/src/lib.rs`; verify with `cargo check --workspace`
- [X] T003 Add `studio-components` and `studio-navigation` workspace members with minimal documented library roots in `Cargo.toml`, `crates/studio-components/Cargo.toml`, `crates/studio-components/src/lib.rs`, `crates/studio-navigation/Cargo.toml`, and `crates/studio-navigation/src/lib.rs`; verify with `cargo check --workspace`
- [X] T004 Add `studio-security` and `studio-actions` workspace members with minimal documented library roots in `Cargo.toml`, `crates/studio-security/Cargo.toml`, `crates/studio-security/src/lib.rs`, `crates/studio-actions/Cargo.toml`, and `crates/studio-actions/src/lib.rs`; verify with `cargo check --workspace`
- [X] T005 [P] Create generated-artifact ownership markers and enforce their directory contracts in `protocol/README.md`, `protocol/fixtures/README.md`, `sdk/assemblyscript/assembly/generated/README.md`, `examples/pos-checkout/assets/README.md`, and `tests/foundation.test.ts`; verify with `bun test tests/foundation.test.ts`
- [X] T006 [P] Add locked formatting, lint, Rust, Bun, no-X11-feature, and artifact-drift jobs in `.github/workflows/ci.yml`, document required Linux packages in `docs/development/BUILDING.md`, and enforce the CI contract in `tests/foundation.test.ts`; verify with `bun test tests/foundation.test.ts`
- [X] T007 Add the repository verification entry point in `scripts/test-all.sh` and expose it from `package.json`; make it run formatting, Clippy, Rust tests, AssemblyScript/TypeScript checks, Bun tests, and the feature-graph check; verify with `bun run test:all`

**Checkpoint**: All approved crate and artifact boundaries exist, dependency locks remain intact,
and the repository-wide verification command is green.

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Prove the Wayland-only platform, authoritative protocol generation, hostile fixture
support, and minimal Wasmtime isolation required by every user story.

**⚠️ CRITICAL**: No user-story implementation starts until this phase is complete.

- [X] T008 [P] Add failing deterministic foundation view-model coverage for text, button, text input, scrolling, popup, focus traversal, accessible labels/states, host-scheduled animation policy, and reduced motion in `crates/studio-app/tests/foundation_gallery.rs`; confirm red with `cargo test -p studio-app --test foundation_gallery` without enabling GPUI `test-support` or X11 [FR-030, FR-031]
- [X] T009 Implement the native foundation gallery, deterministic model, and injectable reduced-motion setting in `crates/studio-app/src/lib.rs`, `crates/studio-app/src/foundation.rs`, and `crates/studio-app/src/main.rs`; make T008 pass and verify with `cargo test -p studio-app` [FR-030, FR-031]
- [X] T010 Add a headless Wayland launch and unsupported-session harness in `scripts/test-headless-wayland.sh`, `tests/platform/wayland_startup.test.ts`, and `.github/workflows/ci.yml`; first confirm the test fails because the harness is absent, then implement it and verify native headless launch plus controlled no-endpoint exit with `bun test tests/platform/wayland_startup.test.ts` [FR-031, SC-009]
- [X] T011 [P] Add failing golden/negative protocol tests for closed envelopes, versions, node kinds, IDs, tree budgets, host-context event ownership with no guest-selectable owner field, patch sequencing, navigation, action, and lifecycle variants in `crates/studio-protocol/tests/protocol_v1.rs` and `protocol/fixtures/protocol-v1/invalid/README.md`; confirm red with `cargo test -p studio-protocol --test protocol_v1` [FR-005, FR-008–FR-017]
- [X] T012 Complete authoritative protocol-v1 types, stable errors, limits, and validators in `crates/studio-protocol/src/lib.rs`, `crates/studio-protocol/src/ui.rs`, `crates/studio-protocol/src/navigation.rs`, `crates/studio-protocol/src/actions.rs`, and `crates/studio-protocol/src/lifecycle.rs`; make T011 pass with `cargo test -p studio-protocol` [FR-005, FR-008–FR-017]
- [X] T013 [P] Add a failing schema-generation drift test in `tests/contracts/generated-artifacts.test.ts` and expected schema inventory in `protocol/schemas/README.md`; confirm red with `bun test tests/contracts/generated-artifacts.test.ts` [FR-005, SC-012]
- [X] T014 Implement Rust JSON Schema export from authoritative protocol types in `crates/studio-protocol/src/bin/generate_schema.rs`, add generation dependencies in `crates/studio-protocol/Cargo.toml`, and write outputs under `protocol/schemas/protocol-v1/`; make T013's schema inventory pass with `cargo run -p studio-protocol --bin generate_schema && bun test tests/contracts/generated-artifacts.test.ts` [FR-005]
- [X] T015 Add failing cross-language fixture tests for every host/guest envelope in `sdk/assemblyscript/tests/generated-contract.test.ts` and `protocol/fixtures/protocol-v1/valid/README.md`; confirm red with `bun test sdk/assemblyscript/tests/generated-contract.test.ts` [FR-005, FR-013]
- [X] T016 Implement deterministic AssemblyScript binding/fixture generation in `scripts/generate-protocol.ts`, `sdk/assemblyscript/assembly/generated/protocol.ts`, and `package.json`; make T015 pass and verify a clean regeneration with `bun run generate:protocol && git diff --exit-code -- protocol sdk/assemblyscript/assembly/generated` [FR-005, SC-012]
- [X] T017 [P] Add failing tests for generating WAT/WASM fixtures with declared imports, exports, memories, tables, loops, traps, and bad ABI calls in `crates/studio-testkit/tests/wasm_fixtures.rs`; confirm red with `cargo test -p studio-testkit --test wasm_fixtures` [FR-004–FR-006]
- [X] T018 Implement deterministic hostile/valid fixture builders in `crates/studio-testkit/src/wasm.rs` and fixture metadata in `crates/studio-testkit/fixtures/README.md`; make T017 pass with `cargo test -p studio-testkit --test wasm_fixtures` [FR-004–FR-006]
- [X] T019 Add failing module-policy tests for no WASI, exactly one allowed import, exactly five allowed exports with fixed signatures, rejection of AssemblyScript runtime/user extras, one bounded memory/table, and disabled proposals in `crates/studio-wasm/tests/module_policy.rs`; confirm red with `cargo test -p studio-wasm --test module_policy` [FR-004–FR-006]
- [X] T020 Implement pinned Wasmtime engine configuration and pre-instantiation module policy in `crates/studio-wasm/Cargo.toml`, `crates/studio-wasm/src/engine.rs`, and `crates/studio-wasm/src/policy.rs`; make T019 pass with `cargo test -p studio-wasm --test module_policy` [FR-004–FR-006]
- [X] T021 Add failing adversarial tests for pointer/length overflow, out-of-bounds access, invalid UTF-8, retained-slice prevention, oversized messages, and non-reentrant `emit` in `crates/studio-wasm/tests/guest_memory.rs`; confirm red with `cargo test -p studio-wasm --test guest_memory` [FR-004–FR-006]
- [X] T022 Implement checked copy-in/copy-out guest memory helpers and the queued `studio_host.emit` bridge in `crates/studio-wasm/src/memory.rs`, `crates/studio-wasm/src/abi.rs`, and `crates/studio-wasm/src/queue.rs`; make T021 pass with `cargo test -p studio-wasm --test guest_memory` [FR-004–FR-006]
- [X] T023 Add failing lifecycle/resource tests for 16 MiB memory, table limits, 15 million initialization fuel, 10 million event fuel, 50 ms epoch interruption, queue limits, traps, and terminal state behavior in `crates/studio-wasm/tests/runtime_limits.rs`; confirm red with `cargo test -p studio-wasm --test runtime_limits` [FR-006, FR-007, SC-005]
- [X] T024 Implement `StoreLimits`, per-call fuel reset, epoch deadline trapping, runtime budgets, and instance lifecycle transitions in `crates/studio-wasm/src/limits.rs`, `crates/studio-wasm/src/instance.rs`, and `crates/studio-wasm/src/lib.rs`; make T023 pass and verify all foundation suites with `cargo test -p studio-wasm -p studio-testkit -p studio-protocol` [FR-006, FR-007, SC-005]

**Checkpoint**: The platform gate, contract source/generation, fixture toolkit, ABI memory boundary,
and Wasmtime resource containment are green. User stories may now be implemented.

---

## Phase 3: User Story 1 — Launch a Trusted Business Plugin (Priority: P1) 🎯 MVP

**Goal**: Verify a production or explicitly selected development bundle before execution, launch
one isolated plugin, and display its initial catalog as accessible native controls on Wayland.

**Independent Test**: Launch a valid signed reference bundle under native Wayland and use its
catalog controls. Mutate the bundle and confirm host-owned rejection. Launch an unsigned explicit
path with developer mode and confirm the persistent warning. Start without Wayland and confirm a
controlled exit.

### Tests for User Story 1

- [X] T025 [P] [US1] Add failing closed-manifest tests for required identity, versions, capabilities, resource declarations, RFC 8785 canonicalization inputs, unknown fields, and host ceilings in `crates/studio-package/tests/manifest_v1.rs`; confirm red with `cargo test -p studio-package --test manifest_v1` [FR-001–FR-003]
- [X] T026 [P] [US1] Add failing archive-adversary and reproducibility tests for traversal, absolute/colliding/duplicate paths, links, compression, non-fixed metadata, extras/comments, ordering, and byte limits in `crates/studio-package/tests/archive_security.rs`; confirm red with `cargo test -p studio-package --test archive_security` [FR-001, FR-002]
- [X] T027 [P] [US1] Add failing integrity/trust tests using RFC 8785 vectors and exact raw 64-byte signatures for manifest/module/asset mutation, publisher/key mismatch, disabled keys, malformed signature length, and invalid signatures in `crates/studio-package/tests/signature_verification.rs`; confirm red with `cargo test -p studio-package --test signature_verification` [FR-001, FR-002]
- [X] T028 [P] [US1] Add failing mount/ownership tests for all-or-nothing initial trees, duplicate IDs, budgets, invalid properties, and one instance namespace in `crates/studio-ui/tests/mount_registry.rs`; confirm red with `cargo test -p studio-ui --test mount_registry` [FR-008, FR-009, FR-013]
- [X] T029 [P] [US1] Add failing native mapping tests for every protocol-v1 layout, display, interaction, and overlay node, including keyboard focus, pointer click/scroll/slider drag, touch-sized targets, labels/states, host-owned secret input, overlays, and owner-checked non-secret events in `crates/studio-components/tests/catalog_mapping.rs`; confirm red with `cargo test -p studio-components --test catalog_mapping` [FR-008, FR-013, FR-030]
- [X] T030 [US1] Implement closed manifest/domain types and validation in `crates/studio-package/src/manifest.rs`, `crates/studio-package/src/error.rs`, and `crates/studio-package/src/lib.rs`; make T025 pass with `cargo test -p studio-package --test manifest_v1` [FR-001–FR-003]
- [X] T031 [US1] Implement bounded streaming archive inspection plus byte-deterministic stored ZIP rules—ordered UTF-8 paths, fixed timestamp/mode, and no extras/comments—in `crates/studio-package/src/archive.rs` and `crates/studio-package/Cargo.toml`; make T026 pass with `cargo test -p studio-package --test archive_security` [FR-001, FR-002]
- [X] T032 [US1] Implement RFC 8785 canonical digest documents, exact raw 64-byte Ed25519 verification, and provisioned trust-store lookup in `crates/studio-package/src/integrity.rs`, `crates/studio-package/src/trust.rs`, and `crates/studio-package/Cargo.toml`; make T027 pass with `cargo test -p studio-package --test signature_verification` [FR-001, FR-002]
- [X] T033 [US1] Implement the instance-owned retained registry and atomic mount validation in `crates/studio-ui/src/node.rs`, `crates/studio-ui/src/registry.rs`, `crates/studio-ui/src/mount.rs`, and `crates/studio-ui/src/lib.rs`; make T028 pass with `cargo test -p studio-ui --test mount_registry` [FR-008, FR-009, FR-013]
- [X] T034 [US1] Audit and vendor only the required pinned `gpui-component` foundation, recording features and deltas in `vendor/gpui-component/`, `docs/upstream/gpui-component-delta.md`, `Cargo.toml`, and `THIRD_PARTY_NOTICES.md`; verify no X11 feature appears with `./scripts/check-no-x11-features.sh` [FR-008, FR-031]
- [X] T035 [US1] Implement Studio-owned native wrappers for every protocol-v1 layout/display/interaction/overlay node plus pointer/touch-sized interaction and host-context event dispatch in `crates/studio-components/src/catalog.rs`, `crates/studio-components/src/events.rs`, `crates/studio-components/src/lib.rs`, and `crates/studio-components/Cargo.toml`; make T029 pass with `cargo test -p studio-components --test catalog_mapping` [FR-008, FR-013, FR-030]
- [X] T036 [US1] Add a failing launch integration test covering `--bundle` with an administrator-provisioned absolute production path, relative/non-file rejection, valid signed launch, mutated rejection, explicit unsigned `--dev`, no-Wayland, and keyboard/pointer catalog input in `tests/integration/plugin_launch.rs`; confirm red with `cargo test --test plugin_launch` [FR-001–FR-009, FR-030, FR-031, SC-001, SC-005, SC-009]
- [X] T037 [US1] Implement absolute regular-file `--bundle` selection, explicit `--dev` selection, bundle-to-policy-to-instance-to-mount orchestration, host-owned load errors, and persistent development warning in `crates/studio-app/src/host.rs`, `crates/studio-app/src/plugin_surface.rs`, `crates/studio-app/src/cli.rs`, `crates/studio-app/src/main.rs`, and `crates/studio-app/Cargo.toml`; make T036 pass with `cargo test --test plugin_launch` [FR-001–FR-009, FR-031]
- [X] T038 [US1] Create and sign the minimal deterministic catalog fixture in `examples/pos-checkout/assembly/index.ts`, `examples/pos-checkout/assets/catalog.json`, `examples/pos-checkout/manifest.json`, `examples/pos-checkout/tests/launch.test.ts`, and `scripts/build-example.ts`; verify the US1 slice with `bun run build:pos && cargo test --test plugin_launch && bun test examples/pos-checkout/tests/launch.test.ts` [FR-032, SC-001]

**Checkpoint**: User Story 1 is a demonstrable MVP. A trusted plugin launches as native UI;
invalid bundles never execute; developer mode is explicit and visibly untrusted.

---

## Phase 4: User Story 2 — Operate a Responsive Catalog and Cart (Priority: P2)

**Goal**: Provide the protocol, native retained-tree mutation, SDK reactivity, and example cart
logic required for targeted, exact, interaction-preserving updates.

**Independent Test**: Add/remove products and edit quantities/discounts against deterministic
catalog data. Verify exact minor-unit totals and that only dependent properties change while
unrelated focus, scroll, and input state remain stable.

### Tests for User Story 2

- [X] T039 [P] [US2] Add failing per-kind property and child-cardinality contract tests for the protocol-v1 component catalog in `crates/studio-protocol/tests/ui_properties_v1.rs`; confirm red with `cargo test -p studio-protocol --test ui_properties_v1` [FR-008–FR-012]
- [X] T040 [P] [US2] Add failing atomic patch tests for update/insert/remove/replace, combined-state budgets, invalid targets/indices, replayed sequence, and rollback in `crates/studio-ui/tests/patch_transaction.rs`; confirm red with `cargo test -p studio-ui --test patch_transaction` [FR-010–FR-012]
- [X] T041 [P] [US2] Add failing native-state preservation tests for text/value/enabled updates, focus, scroll, input buffers, and unrelated ancestor identity in `crates/studio-components/tests/targeted_updates.rs`; confirm red with `cargo test -p studio-components --test targeted_updates` [FR-010–FR-012, SC-002]
- [X] T042 [P] [US2] Add failing AssemblyScript tests for state dependency tracking, derived memoization, effects, batching, flush order, disposal, and cycle limits in `sdk/assemblyscript/tests/reactivity.test.ts`; confirm red with `bun test sdk/assemblyscript/tests/reactivity.test.ts` [FR-014, FR-015, SC-010]
- [X] T043 [P] [US2] Add failing SDK widget-binding tests proving one mount followed by minimal property/structural patches in `sdk/assemblyscript/tests/widget-bindings.test.ts`; confirm red with `bun test sdk/assemblyscript/tests/widget-bindings.test.ts` [FR-008, FR-010, FR-014]
- [X] T044 [US2] Implement closed property values, per-kind property schemas, child rules, and validation errors in `crates/studio-protocol/src/properties.rs`, `crates/studio-protocol/src/ui.rs`, and `crates/studio-protocol/src/error.rs`; make T039 pass and regenerate artifacts with `cargo test -p studio-protocol --test ui_properties_v1 && bun run generate:protocol` [FR-008–FR-012]
- [X] T045 [US2] Implement staged update transactions, sequence enforcement, combined-tree validation, and all-or-none commit in `crates/studio-ui/src/patch.rs`, `crates/studio-ui/src/transaction.rs`, and `crates/studio-ui/src/registry.rs`; make T040 pass with `cargo test -p studio-ui --test patch_transaction` [FR-010, FR-011]
- [X] T046 [US2] Implement targeted native property mutation/invalidation and state-preserving structural reconciliation in `crates/studio-components/src/update.rs`, `crates/studio-components/src/state.rs`, and `crates/studio-components/src/lib.rs`; make T041 pass with `cargo test -p studio-components --test targeted_updates` [FR-010–FR-012]
- [X] T047 [US2] Implement AssemblyScript scheduler, state cells, dependency tracking, batching, and bounded flush queue in `sdk/assemblyscript/assembly/reactivity/scheduler.ts`, `sdk/assemblyscript/assembly/reactivity/state.ts`, and `sdk/assemblyscript/assembly/reactivity/batch.ts`; make the state/batch subset of T042 pass with `bun test sdk/assemblyscript/tests/reactivity.test.ts` [FR-014, FR-015]
- [X] T048 [US2] Implement derived/effect memoization, ownership disposal, and cycle detection in `sdk/assemblyscript/assembly/reactivity/derived.ts`, `sdk/assemblyscript/assembly/reactivity/effect.ts`, and `sdk/assemblyscript/assembly/reactivity/owner.ts`; make all of T042 pass with `bun test sdk/assemblyscript/tests/reactivity.test.ts` [FR-014, FR-015, SC-010]
- [X] T049 [US2] Implement typed layout/display/interaction builders, stable IDs, mount serialization, property bindings, and structural patch emission in `sdk/assemblyscript/assembly/widgets.ts`, `sdk/assemblyscript/assembly/bindings.ts`, `sdk/assemblyscript/assembly/runtime.ts`, and `sdk/assemblyscript/assembly/index.ts`; make T043 pass with `bun test sdk/assemblyscript/tests/widget-bindings.test.ts` [FR-008, FR-010, FR-014]
- [X] T050 [US2] Add a failing catalog/cart integration test for search, add/remove, quantities, batched discount changes, exact currency, and preserved interaction state in `tests/integration/catalog_cart.rs`; confirm red with `cargo test --test catalog_cart` [FR-010–FR-015, FR-026, SC-002]
- [X] T051 [US2] Implement integer `Money`, deterministic product/cart state, derived totals, and fine-grained bindings in `examples/pos-checkout/assembly/money.ts`, `examples/pos-checkout/assembly/catalog.ts`, `examples/pos-checkout/assembly/cart.ts`, and `examples/pos-checkout/assembly/index.ts`; make T050 pass with `bun run build:pos && cargo test --test catalog_cart` [FR-014, FR-015, FR-026]
- [X] T052 [US2] Add property and patch performance instrumentation and idle-message assertions in `crates/studio-ui/benches/patches.rs`, `tests/performance/catalog_latency.rs`, and `tests/integration/idle_plugin.rs`; verify the US2 slice with `cargo test --test catalog_cart --test idle_plugin && cargo bench -p studio-ui --bench patches` [SC-002, SC-010]

**Checkpoint**: User Stories 1 and 2 both pass. The reference catalog/cart is responsive,
currency-exact, and driven by targeted retained updates rather than full guest rerenders.

---

## Phase 5: User Story 3 — Complete a Protected Simulated Payment (Priority: P3)

**Goal**: Capture PIN input in host memory, expose only scoped opaque references, show trusted
confirmation, and execute deterministic idempotent simulated payments without secret leakage.

**Independent Test**: Run every simulator outcome and all wrong-owner/session/purpose/expiry/reuse
cases, then scan every plugin-visible and persisted artifact for the raw PIN fixture.

### Tests for User Story 3

- [X] T053 [P] [US3] Add failing principal/capability tests for exact publisher/plugin/bundle/instance identity, manifest declaration, host policy, unknown operations, and queue ceilings in `crates/studio-security/tests/principal_policy.rs`; confirm red with `cargo test -p studio-security --test principal_policy` [FR-004, FR-005, FR-020, FR-021]
- [X] T054 [P] [US3] Add failing opaque-reference tests for 256-bit randomness, exact scoping, 120-second expiry, single use, revocation, non-oracular errors, and zeroization hooks in `crates/studio-security/tests/opaque_handles.rs`; confirm red with `cargo test -p studio-security --test opaque_handles` [FR-019–FR-021, FR-029]
- [X] T055 [P] [US3] Add failing host-owned secret-input tests proving raw bytes never enter protocol events, guest memory, widget snapshots, or diagnostics in `crates/studio-components/tests/secret_input.rs`; confirm red with `cargo test -p studio-components --test secret_input` [FR-019, FR-020, FR-029, SC-006]
- [X] T056 [P] [US3] Add failing confirmation-state tests for merchant/publisher identity, amount/currency snapshot binding, cancellation, expiry during confirmation, and post-confirmation cart mutation in `crates/studio-actions/tests/payment_confirmation.rs`; confirm red with `cargo test -p studio-actions --test payment_confirmation` [FR-022, FR-023]
- [X] T057 [P] [US3] Add failing payment tests for missing authorization, four deterministic suffix outcomes, host-process-lifetime replay, conflicting replay, 10,000-record capacity, retained replay at capacity, rejection of new keys at capacity, no network access, and stable safe errors in `crates/studio-actions/tests/payment_simulator.rs`; confirm red with `cargo test -p studio-actions --test payment_simulator` [FR-024–FR-026, SC-004, SC-007]
- [X] T058 [US3] Implement immutable principal identities and deny-by-default capability decisions in `crates/studio-security/src/principal.rs`, `crates/studio-security/src/capability.rs`, and `crates/studio-security/src/lib.rs`; make T053 pass with `cargo test -p studio-security --test principal_policy` [FR-004, FR-005, FR-020, FR-021]
- [X] T059 [US3] Implement zeroizing secret records, CSPRNG opaque IDs, scoped resolution, monotonic expiry, single-use transitions, and revocation in `crates/studio-security/src/secret.rs`, `crates/studio-security/src/registry.rs`, and `crates/studio-security/Cargo.toml`; make T054 pass with `cargo test -p studio-security --test opaque_handles` [FR-019–FR-021, FR-029]
- [X] T060 [US3] Implement Studio-owned `SecretInput` storage/event projection and teardown hooks in `crates/studio-components/src/secret_input.rs`, `crates/studio-components/src/events.rs`, and `crates/studio-components/src/state.rs`; make T055 pass with `cargo test -p studio-components --test secret_input` [FR-019, FR-020, FR-029]
- [X] T061 [US3] Implement checkout confirmation snapshot and host-owned confirmation surface model in `crates/studio-actions/src/checkout.rs`, `crates/studio-actions/src/confirmation.rs`, and `crates/studio-actions/src/lib.rs`; make T056 pass with `cargo test -p studio-actions --test payment_confirmation` [FR-022, FR-023, FR-026]
- [X] T062 [US3] Implement the deterministic in-memory payment simulator and 10,000-record process-lifetime idempotency registry with no eviction of terminal records in `crates/studio-actions/src/payment.rs`, `crates/studio-actions/src/idempotency.rs`, and `crates/studio-actions/src/error.rs`; make T057 pass with `cargo test -p studio-actions --test payment_simulator` [FR-024–FR-026, SC-004, SC-007]
- [X] T063 [P] [US3] Add a failing redaction test that injects raw secrets and active references into guest logs, diagnostics, snapshots, action errors, receipts, and file-output attempts in `tests/security/redaction.rs`; confirm red with `cargo test --test redaction` [FR-029, SC-006]
- [X] T064 [US3] Implement centralized sensitive-value rejection/redaction and safe diagnostic formatting in `crates/studio-security/src/redaction.rs`, `crates/studio-wasm/src/diagnostic.rs`, and `crates/studio-app/src/diagnostic.rs`; make T063 pass with `cargo test --test redaction` [FR-029, SC-006]
- [X] T065 [US3] Add a failing protected-payment end-to-end test covering no PIN, opaque readiness, trusted confirmation, all invalid handles, bound amount, all outcomes, and idempotent replay in `tests/integration/protected_payment.rs`; confirm red with `cargo test --test protected_payment` [FR-019–FR-026, SC-004, SC-006, SC-007]
- [X] T066 [US3] Implement checkout/payment SDK helpers and reference checkout screen in `sdk/assemblyscript/assembly/actions.ts`, `examples/pos-checkout/assembly/checkout.ts`, `examples/pos-checkout/assembly/index.ts`, `crates/studio-app/src/confirmation_surface.rs`, and `crates/studio-app/src/host.rs`; make T065 pass with `bun run build:pos && cargo test --test protected_payment` [FR-019–FR-026]

**Checkpoint**: User Story 3 passes independently against simulator fixtures, and the raw PIN is
absent from every guest-visible, logged, snapshotted, receipted, and persisted artifact.

---

## Phase 6: User Story 4 — Navigate, Recover, and Produce a Receipt (Priority: P4)

**Goal**: Deliver host-validated routes, stack operations, guards, reduced-motion transitions,
safe payment recovery, immutable receipts, and host-owned printer previews.

**Independent Test**: Traverse catalog through receipt with every payment result; confirm stack
and guard behavior, reduced-motion equivalence, retry safety, receipt integrity, and structured
print preview.

### Tests for User Story 4

- [X] T067 [P] [US4] Add failing route matcher tests for nested/static/parameterized/not-found routes, canonical parameters, ambiguous declarations, and lazy creation in `crates/studio-navigation/tests/route_tree.rs`; confirm red with `cargo test -p studio-navigation --test route_tree` [FR-016, FR-017]
- [X] T068 [P] [US4] Add failing stack/guard tests for push/replace/pop/pop-to/reset, depth 32, ownership, pending-payment denial, confirmation, and 50 ms timeout in `crates/studio-navigation/tests/navigation_stack.rs`; confirm red with `cargo test -p studio-navigation --test navigation_stack` [FR-016, FR-017]
- [X] T069 [P] [US4] Add failing transition tests for push/pop/replace timing, deterministic final state, host-clock scheduling, interruption, and zero-duration reduced motion in `crates/studio-navigation/tests/transitions.rs`; confirm red with `cargo test -p studio-navigation --test transitions` [FR-018]
- [X] T070 [P] [US4] Add failing receipt tests for approved-only creation, exact confirmed money, merchant/lines/result/time consistency, immutability, and secret-free schema in `crates/studio-actions/tests/receipts.rs`; confirm red with `cargo test -p studio-actions --test receipts` [FR-026, FR-027, FR-029]
- [X] T071 [P] [US4] Add failing printer tests for structured approved receipts, ownership, duplicate requests, one preview job, and rejection of raw/device-control bytes in `crates/studio-actions/tests/printer_simulator.rs`; confirm red with `cargo test -p studio-actions --test printer_simulator` [FR-028]
- [X] T072 [US4] Implement declared route parsing/matching, typed parameters, lazy factories, and not-found resolution in `crates/studio-navigation/src/route.rs`, `crates/studio-navigation/src/tree.rs`, and `crates/studio-navigation/src/lib.rs`; make T067 pass with `cargo test -p studio-navigation --test route_tree` [FR-016, FR-017]
- [X] T073 [US4] Implement the instance-owned bounded stack, atomic commands, guard protocol, pending-payment policy, and timeout handling in `crates/studio-navigation/src/stack.rs`, `crates/studio-navigation/src/guard.rs`, and `crates/studio-navigation/src/error.rs`; make T068 pass with `cargo test -p studio-navigation --test navigation_stack` [FR-016, FR-017]
- [X] T074 [US4] Implement host-clock route/property transitions and reduced-motion resolution in `crates/studio-navigation/src/transition.rs`, `crates/studio-components/src/transition.rs`, and `crates/studio-app/src/preferences.rs`; make T069 pass with `cargo test -p studio-navigation --test transitions` [FR-018]
- [X] T075 [US4] Implement immutable structured receipts from approved confirmation snapshots in `crates/studio-actions/src/receipt.rs` and extend action schemas in `crates/studio-protocol/src/actions.rs`; make T070 pass and regenerate contracts with `cargo test -p studio-actions --test receipts && bun run generate:protocol` [FR-026, FR-027, FR-029]
- [X] T076 [US4] Implement the in-memory structured printer simulator and host preview model in `crates/studio-actions/src/printer.rs`, `crates/studio-actions/src/lib.rs`, and `crates/studio-app/src/print_preview.rs`; make T071 pass with `cargo test -p studio-actions --test printer_simulator` [FR-028]
- [X] T077 [P] [US4] Add a failing navigation/recovery integration test for route-local state restoration, pending-payment exit, decline/timeout/unavailable retry, reduced motion, receipt, and preview in `tests/integration/checkout_navigation.rs`; confirm red with `cargo test --test checkout_navigation` [FR-016–FR-018, FR-024, FR-027, FR-028]
- [X] T078 [US4] Implement SDK route declarations/commands/guards and POS cart/payment/receipt screens in `sdk/assemblyscript/assembly/navigation.ts`, `examples/pos-checkout/assembly/routes.ts`, `examples/pos-checkout/assembly/payment.ts`, `examples/pos-checkout/assembly/receipt.ts`, and `examples/pos-checkout/assembly/index.ts`; make T077 pass with `bun run build:pos && cargo test --test checkout_navigation` [FR-016–FR-018, FR-032]
- [X] T079 [US4] Add a failing keyboard-only catalog-to-receipt acceptance test with all payment outcomes and print preview in `tests/e2e/pos_checkout.rs`; confirm red with `cargo test --test pos_checkout` [FR-030, FR-032, SC-003, SC-004, SC-008]
- [X] T080 [US4] Compose navigation, trusted overlays, payment recovery, receipt, and preview into the native shell in `crates/studio-app/src/router.rs`, `crates/studio-app/src/action_dispatch.rs`, `crates/studio-app/src/plugin_surface.rs`, and `crates/studio-app/src/host.rs`; make T079 pass with `cargo test --test pos_checkout` [FR-016–FR-032]

**Checkpoint**: User Story 4 completes the operator journey from catalog through a consistent
receipt and host-owned print preview, including safe failure paths and reduced motion.

---

## Phase 7: User Story 5 — Build a Reactive Plugin Safely (Priority: P5)

**Goal**: Make the supported component, reactivity, event, navigation, action, packaging,
diagnostic, and fresh-restart behavior usable and testable by an external AssemblyScript author.

**Independent Test**: Build a starter counter/total plugin from repository documentation in under
ten minutes, package and open it in developer mode, observe targeted updates, trigger invalid
operations and a trap, then manually restart into a fresh instance.

### Tests for User Story 5

- [X] T081 [P] [US5] Add failing SDK component-catalog tests for all layout/display/interaction/overlay builders, typed properties, stable IDs, events, and `Container`→`Box` aliasing in `sdk/assemblyscript/tests/component-catalog.test.ts`; confirm red with `bun test sdk/assemblyscript/tests/component-catalog.test.ts` [FR-008, FR-014]
- [X] T082 [P] [US5] Add failing SDK helper tests for non-secret events, navigation results, asynchronous action correlation, lifecycle messages, and pending-request ceilings in `sdk/assemblyscript/tests/host-helpers.test.ts`; confirm red with `bun test sdk/assemblyscript/tests/host-helpers.test.ts` [FR-005, FR-013–FR-017]
- [X] T083 [P] [US5] Add failing deterministic packager CLI tests for RFC 8785 signing input, stored ZIP ordering/fixed metadata, raw signatures, explicit dev bundles, byte-identical repeated output, and safe diagnostics in `crates/studio-package/tests/packager_cli.rs`; confirm red with `cargo test -p studio-package --test packager_cli` [FR-001–FR-003, SC-011]
- [X] T084 [P] [US5] Add failing diagnostic tests for unknown components/properties/routes/events/capabilities, malformed batches, lifecycle misuse, and guaranteed no partial UI mutation in `tests/integration/developer_errors.rs`; confirm red with `cargo test --test developer_errors` [FR-005, FR-007–FR-017, FR-029]
- [X] T085 [P] [US5] Add failing trap/resource recovery tests proving host responsiveness, host-owned failure UI, manual-only restart, new instance identity, revoked handles, cancelled actions, and no restored plugin state in `tests/integration/plugin_recovery.rs`; confirm red with `cargo test --test plugin_recovery` [FR-006, FR-007, SC-005]
- [X] T086 [US5] Complete the AssemblyScript component catalog and closed typed property/event builders in `sdk/assemblyscript/assembly/components/layout.ts`, `sdk/assemblyscript/assembly/components/display.ts`, `sdk/assemblyscript/assembly/components/interaction.ts`, `sdk/assemblyscript/assembly/components/overlay.ts`, and `sdk/assemblyscript/assembly/widgets.ts`; make T081 pass with `bun test sdk/assemblyscript/tests/component-catalog.test.ts` [FR-008, FR-014]
- [X] T087 [US5] Implement SDK event registration, navigation/action correlation, lifecycle dispatch, and bounded pending requests in `sdk/assemblyscript/assembly/events.ts`, `sdk/assemblyscript/assembly/navigation.ts`, `sdk/assemblyscript/assembly/actions.ts`, and `sdk/assemblyscript/assembly/runtime.ts`; make T082 pass with `bun test sdk/assemblyscript/tests/host-helpers.test.ts` [FR-005, FR-013–FR-017]
- [X] T088 [US5] Implement RFC 8785 signing and byte-deterministic stored-ZIP `.studio` packaging CLI with raw signatures and explicit unsigned development output in `crates/studio-package/src/bin/studio-pack.rs`, `crates/studio-package/src/pack.rs`, `crates/studio-package/src/sign.rs`, and `crates/studio-package/Cargo.toml`; make T083 pass with `cargo test -p studio-package --test packager_cli` [FR-001–FR-003, SC-011]
- [X] T089 [US5] Implement stable developer-facing validation/lifecycle diagnostics with redacted context in `crates/studio-protocol/src/error.rs`, `crates/studio-ui/src/error.rs`, `crates/studio-app/src/diagnostic.rs`, and `sdk/assemblyscript/assembly/errors.ts`; make T084 pass with `cargo test --test developer_errors` [FR-005, FR-007, FR-029]
- [X] T090 [US5] Implement host-owned termination surface and manual fresh-instance restart orchestration in `crates/studio-app/src/failure_surface.rs`, `crates/studio-app/src/host.rs`, `crates/studio-wasm/src/instance.rs`, and `crates/studio-security/src/registry.rs`; make T085 pass with `cargo test --test plugin_recovery` [FR-006, FR-007, SC-005]
- [X] T091 [P] [US5] Add a failing documented starter-plugin smoke test that builds a counter/derived-total UI, packages it, launches in dev mode, applies one targeted patch, and surfaces one invalid operation in `tests/e2e/starter_plugin.test.ts`; confirm red with `bun test tests/e2e/starter_plugin.test.ts` [FR-008, FR-010, FR-014, SC-011]
- [X] T092 [US5] Create the documented starter plugin and SDK authoring guide in `examples/starter/assembly/index.ts`, `examples/starter/manifest.json`, `examples/starter/package.json`, `docs/sdk/GETTING_STARTED.md`, and `package.json`; make T091 pass with `bun test tests/e2e/starter_plugin.test.ts` [FR-008, FR-014, SC-011]
- [X] T093 [US5] Add a timed developer-experience validation script and release checklist entry in `scripts/test-starter-quickstart.sh`, `docs/sdk/GETTING_STARTED.md`, and `docs/ROADMAP.md`; verify the US5 slice with `./scripts/test-starter-quickstart.sh` and record whether the documented flow completes within ten minutes [SC-011]

**Checkpoint**: The platform is usable through the supported SDK and packager, failures are
actionable but non-sensitive, and a terminated plugin can only restart as a fresh instance.

---

## Phase 8: Polish & Cross-Cutting Hardening

**Purpose**: Prove cross-story shutdown, accessibility, performance, provenance, fuzz resistance,
release platform constraints, and complete requirement traceability.

- [X] T094 Add a failing compositor-loss integration test with an active plugin, pending payment, live opaque reference, UI tree, and navigation stack in `tests/integration/compositor_disconnect.rs`; confirm red with `cargo test --test compositor_disconnect` [FR-031]
- [X] T095 Implement ordered compositor-loss shutdown—cancel actions, revoke secrets, terminate instances, close native state, and exit without restoration—in `crates/studio-app/src/shutdown.rs`, `crates/studio-app/src/host.rs`, `crates/studio-actions/src/lib.rs`, and `crates/studio-security/src/registry.rs`; make T094 pass and retain T010 with `cargo test --test compositor_disconnect && bun test tests/platform/wayland_startup.test.ts` [FR-031]
- [X] T096 [P] Add bounded fuzz targets and seed corpora for protocol decode/validation, patch transactions, bundle archives/manifests, and action payloads in `fuzz/Cargo.toml`, `fuzz/fuzz_targets/protocol_decode.rs`, `fuzz/fuzz_targets/patch_transaction.rs`, `fuzz/fuzz_targets/bundle_parse.rs`, and `fuzz/fuzz_targets/action_payload.rs`; verify 60-second smoke runs per target [FR-002, FR-005, FR-006, SC-005]
- [X] T097 [P] Add `STUDIO-BENCH-1` release benchmarks using 5+30 launch samples and a fixed 10-warm-up/100-sample catalog-cart-navigation corpus measured event-to-present for warm first frame, interaction latency, property/batch patches, 60 FPS transitions, and idle work in `benches/acceptance.rs`, `scripts/benchmark-acceptance.sh`, and `docs/performance/BASELINE.md`; verify with `./scripts/benchmark-acceptance.sh` [SC-001, SC-002, SC-010]
- [X] T098 [P] Strengthen release platform checks for dependency features, `libX11`/`libxcb` linkage, `DISPLAY`-unset build, native headless Wayland launch, and no XWayland process in `scripts/check-no-x11-features.sh`, `scripts/check-no-x11.sh`, `scripts/test-headless-wayland.sh`, and `.github/workflows/ci.yml`; verify all three scripts against `target/release/studio-app` [FR-031, SC-009]
- [X] T099 [P] Add full keyboard/focus/label/state and reduced-motion acceptance coverage for every reference checkout screen in `tests/e2e/accessibility.rs` and document unavoidable manual native checks in `docs/accessibility/ACCEPTANCE.md`; verify with `cargo test --test accessibility` [FR-018, FR-030, SC-008]
- [X] T100 [P] Audit final Cargo/Bun dependencies, the `gpui-component` fork, and every Oxide-derived item; update `docs/upstream/oxide-audit.md`, `docs/upstream/gpui-component-delta.md`, `THIRD_PARTY_NOTICES.md`, and `docs/security/DEPENDENCY_AUDIT.md`; verify license/provenance assertions with `bun test tests/foundation.test.ts`
- [X] T101 Update security assumptions, failure flows, compositor shutdown, opaque-reference lifecycle, simulator boundaries, and residual risks in `docs/security/THREAT_MODEL.md` and `docs/architecture/ADR-0001-system-boundaries.md`; verify contract links with `bun test tests/foundation.test.ts`
- [X] T102 Build the FR/SC-to-test/manual-check matrix, its completeness test, and milestone release checklist in `specs/001-secure-plugin-runtime/traceability.md`, `tests/traceability.test.ts`, and `docs/RELEASE_CHECKLIST.md`; first confirm the test reports missing evidence, then make every `FR-001`–`FR-032` and `SC-001`–`SC-012` entry pass with `bun test tests/traceability.test.ts` [SC-012]
- [X] T103 Run every command and scenario in `specs/001-secure-plugin-runtime/quickstart.md` and record dated results plus baseline hardware in `specs/001-secure-plugin-runtime/validation-report.md`; verify `bun run test:all`, release no-X11 checks, headless Wayland, security suites, fuzz smoke, and acceptance benchmarks are all green [SC-001–SC-012]

**Final Checkpoint**: All five user stories, cross-story shutdown, security, platform, accessibility,
performance, provenance, and traceability evidence pass on the documented baseline.

---

## Dependencies & Execution Order

### Phase Dependencies

```text
Phase 1 Setup
    |
    v
Phase 2 Foundation (hard blocker)
    |
    v
US1 Trusted Launch (MVP)
    |
    v
US2 Catalog/Cart + Retained Reactivity
    |
    v
US3 Protected Payment
    |
    v
US4 Navigation/Receipt
    |
    v
US5 Developer Experience/Recovery
    |
    v
Cross-Cutting Hardening and Release Evidence
```

- **Setup** has no prior dependency. T001–T004 are sequential because each edits `Cargo.toml`;
  T005 and T006 may run in parallel, then T007 closes the phase.
- **Foundation** depends on Setup. Test tasks T008, T011, T013, T015, and T017 can begin in
  parallel. Sandbox tests T019/T021/T023 then execute in their implementation order.
- **US1** depends on Foundation and is the suggested MVP. Bundle test tasks T025–T027 and UI test
  tasks T028–T029 may run in parallel before their corresponding implementations.
- **US2** depends on US1's mount/renderer path. Its protocol/UI and SDK test streams can run in
  parallel before integration.
- **US3** depends on the US1 action bridge and US2 checkout/cart money model. Security,
  confirmation, payment, and redaction test streams are separable until the E2E join.
- **US4** depends on the cart and payment state from US2/US3. Route, transition, receipt, and
  printer streams can run in parallel before checkout integration.
- **US5** depends on the runtime, UI, SDK, navigation, and actions it documents, though packaging
  CLI and catalog tests can begin earlier once US1/US2 contracts stabilize.
- **Polish** depends on all stories selected for the release. T096–T101 are parallel after T095;
  T102 and T103 close milestone evidence.

### User Story Independence

| Story | Independently testable result | Required earlier product slice |
| --- | --- | --- |
| US1 | Trusted bundle launches native catalog; invalid/dev/no-Wayland paths behave safely | Foundation only |
| US2 | Deterministic catalog/cart emits targeted exact patches and preserves interaction state | US1 mount and native event path |
| US3 | Protected simulator payment completes without raw PIN exposure | US1 action bridge + US2 cart money |
| US4 | Navigation/recovery/receipt/preview completes for all outcomes | US2 cart + US3 payment state |
| US5 | External author builds, packages, diagnoses, traps, and freshly restarts a plugin | Stable contracts/runtime/SDK from US1–US4 |

### Within Every Behavioral Pair

1. Add the named focused test and run it.
2. Confirm failure for the missing behavior—not syntax, fixture, or environment failure.
3. Implement only the bounded behavior in the following task.
4. Run the focused test and affected crate/package tests.
5. Refactor only while those suites remain green.
6. Update the active spec before continuing if the contract or threat assumptions change.

## Parallel Execution Examples

### User Story 1

```text
T025 manifest contract tests       || T028 mount registry tests
T026 archive adversary tests       || T029 native catalog mapping tests
T027 signature/trust tests
```

Then implement T030–T035 in dependency order and join at T036–T038.

### User Story 2

```text
T039 property schema tests         || T042 reactivity tests
T040 patch transaction tests       || T043 widget-binding tests
T041 targeted native-state tests
```

Join after T044–T049 for T050–T052.

### User Story 3

```text
T053 principal/capability tests    || T055 secret-input tests
T054 opaque-reference tests        || T056 confirmation tests
T057 payment simulator tests       || T063 redaction tests
```

Join after T058–T064 for T065–T066.

### User Story 4

```text
T067 route matcher tests           || T069 transition tests
T068 stack/guard tests             || T070 receipt tests
                                    || T071 printer tests
```

Join after T072–T076 for T077–T080.

### User Story 5

```text
T081 component catalog tests       || T083 packager CLI tests
T082 host-helper tests             || T084 developer diagnostics tests
                                    || T085 recovery tests
```

Join after T086–T090 for T091–T093.

## Implementation Strategy

### MVP First

1. Complete Setup and Foundation.
2. Complete User Story 1 through T038.
3. Stop and execute its independent launch test under native/headless Wayland.
4. Demonstrate valid signed launch, altered-bundle rejection, explicit dev mode, native input, and
   controlled unsupported-session behavior before expanding the product.

### Incremental Delivery

1. **US1** proves package trust, sandbox launch, native mount, and Wayland policy.
2. **US2** proves the retained patch/reactivity performance model with a useful cart.
3. **US3** proves the opaque-secret and capability-mediated payment boundary.
4. **US4** completes operator navigation, recovery, receipt, and print preview.
5. **US5** makes the platform reproducibly usable by external plugin authors.
6. **Hardening** proves the integrated release against adversarial, platform, performance,
   accessibility, provenance, and traceability requirements.

At every checkpoint, keep all earlier story tests green. Do not begin real networking, hardware,
guest persistence, web rendering, X11 support, or marketplace work; each is outside this feature.

## Task Scope Notes

- Tasks touch no more than five named files except vendored upstream contents, whose changes are
  bounded by the audit task and delta ledger.
- `[P]` means file-independent at that point in the graph, not permission to bypass prerequisite
  phases or the red-test requirement.
- Generated files are changed through their generator tasks and checked for clean regeneration.
- Documentation-only tasks use link, schema, provenance, or consistency checks instead of an
  artificial runtime test.
- Commit after each task or small Red-Green-Refactor pair so failures and rollbacks remain narrow.

## Phase 9: Convergence

- [X] T104 Wire the implemented `CheckoutRouter`/`NativeCheckoutShell` route state, trusted checkout overlays, receipt, and print-preview surfaces into the live GPUI window selected by `crates/studio-app/src/main.rs` and `crates/studio-app/src/foundation.rs`, and add a native-shell acceptance test proving visible catalog → cart → checkout → receipt route transitions and recovery (`FR-016–FR-018`, `FR-027–FR-032`, `US4/AC1–AC6`)
