# Research: Studio Script and Embedded Development Host

**Date**: 2026-09-03 · Resolves every plan-level unknown for [spec.md](spec.md).

## R1. How `rsvelte` integrates (git dependency, fenced adapter)

**Decision**: Depend on the `rsvelte` workspace at pinned rev
`b64aed9c611557649ebd6428455cfe874255356e` (main, 2026-09-03; MIT license, proprietary-safe
with the copyright notice preserved) via vendored sources under `vendor/rsvelte/` (see `vendor/rsvelte/UPSTREAM.md`):
`rsvelte` (facade: `Engine`, `PreparedComponent`, `fingerprint`) **and** `rsvelte_core`
(parse API: template tree with spans), both `default-features = false`, no `projection`
feature. All usage lives in one new module, `studio-script/src/rsvelte_adapter.rs`; no
`rsvelte` type crosses the `studio-script` boundary. Never call `compile()`/`compile_both()`
(they emit forbidden JavaScript); use `prepare()` for analysis facts and diagnostics, and
`rsvelte_core`'s parse for the template tree the lowering needs. Copy the upstream workspace's
`oxc` git `[patch]` table for type unification, or accept crates.io `oxc 0.146` — the tasks pin
this down before integration.

**Rationale**: The facade deliberately hides its AST (stability promise), while the template
tree needed for lowering lives one layer down in pre-1.0 `rsvelte_core` — fenced to one adapter
module, exactly as the design doc prescribes. The crate is pure Rust (`#![forbid(unsafe_code)]`
on the facade), has no Node/WASM/network runtime requirements with default features, and its
`EngineFingerprint` (facade/compiler/svelte/schema versions) feeds our artifact cache keys.

A Cargo git dependency was attempted first and rejected: `cargo fetch` cannot resolve a nested
fixture submodule with an SSH URL (`src-tauri/lib/portable-vocal-remover`), so Cargo cannot
reproduce the checkout. Vendoring five crates (`rsvelte`, `rsvelte_core`, `rsvelte_esrap`,
`rsvelte_ast_equiv`, `rsvelte_projection`) under `vendor/rsvelte/` with byte-identical sources
plus manifest-only lint deltas is the repo's established pattern (see `vendor/gpui-component`).

**Alternatives considered**: facade-only dependency (rejected: template tree is inaccessible, so
lowering to Studio IR is impossible); cloning the parser surface into Studio-owned code
(rejected: second compiler project with no compatibility oracle, and compatibility fixtures
would have nothing authoritative to compare against); driving npm `@rsvelte/compiler` from Bun
(rejected: routes compilation through generated artifacts and hides the tree behind another
process boundary).

## R2. IR evolution: v1 skeleton → v2 dynamic model

**Decision**: Keep the existing `StudioIrModule`/`IrScreen`/`IrNode`/`IrProperty` static model
working (its lowering, ASC emitter, and `simulate_event` oracle stay green) and extend it to
the design-doc v2: component definitions with typed props, `StateSlot`/`DerivedSlot` vectors,
`EventHandler` list, and template `If`/`Each`/`Interpolation` nodes with typed `Expression`
(closed: literals, prop/state/derived/local reads, unary/binary/conditional, arrays/records,
approved calls). Bump `STUDIO_IR_VERSION` to 2. Raw source text is never an executable node;
unsupported constructs produce diagnostics. Stable IDs derive from module identity plus
structural/source identity.

**Rationale**: The v1 static path already proves IR → ASC → Wasm → host works end to end;
v2 adds exactly the dynamic nodes the evaluator and hot swap need without breaking that proven
path. One version bump keeps every consumer honest.

**Alternatives considered**: a parallel IR crate (rejected: splits the lowering/emitter and
duplicates the versioning story); untyped JSON IR (rejected: loses swap-compatibility checking
and differential provability).

## R3. Portable-subset validator placement and codes

**Decision**: A new `studio-script/src/validate.rs` owns portable-subset and catalog validation
with a new additive `STUDIO3xx` family (subset, types, runes, approved calls, catalog kinds,
routes, assets), following the existing `STUDIO0xx` parser and `STUDIO2xx` lowerer patterns.
Catalog resolution uses the closed 100-kind `NodeKind` set (feature 002) plus the SDK surface.
The single canonical `number` decision (language has one numeric type; host boundary maps
integral i64-range values to Integer, others to Decimal) lives in `types.rs` and is enforced
here. Money-exactness stays with explicit formatting helpers, never float math.

**Rationale**: One validator as the final authority before either backend — exactly the
documented architecture — with codes in the established style so existing diagnostic tooling
consumes them unchanged.

**Alternatives considered**: pushing validation into the lowering passes (rejected: two places to
keep consistent; differential equivalence would need to compare rejection behavior twice).

## R4. Rust evaluator and hot swap

**Decision**: New `studio-script/src/eval/` interprets v2 IR: `mount(module) -> MountTree` plus
`dispatch(module, &HostEvent) -> Option<PatchBatch-ish GuestMessage>` over the same
`UiRegistry::mount/apply_patch` and `HostEventDispatcher` seams the Wasm path uses
(`studio-app`'s `process_input` is the behavioral model to mirror). New
`studio-script/src/hot_swap.rs` implements `replace_module_atomically` per the swap contract:
parse/validate/lower before touching live state, match by stable identity, preserve only when
stable ID **and** `StudioType` match (one numeric type, so numeric edits never break
compatibility; genuine type changes reset to the slot initializer), dispose the rest, roll back
on host validation failure, and reevaluate dependents in dependency order.

**Rationale**: The evaluator shares the exact registry/dispatcher/event vocabulary as
production, which is what makes differential equivalence meaningful rather than aspirational.
The existing `simulate_event` becomes one leg of the differential harness.

**Alternatives considered**: interpreting to a shadow DOM-like tree (rejected: second rendering
model to keep consistent); hot swap at the GPUI widget layer (rejected: bypasses the retained
identity model the host guarantees).

## R5. AssemblyScript backend shape

**Decision**: Extend the existing `assemblyscript::emit` from its closed static module
(`MOUNT_PAYLOAD` + `includes()` dispatch) to generated component modules that import the SDK
widget/component factories, plus a bootstrap materialized only for ASC builds and the reserved
virtual modules (`@studio/generated/routes`, `@studio/generated/assets`, `@studio/dev-runtime`,
`@studio/bootstrap`). Widen the generated `NODE_KIND_*` constants past their current ~27-kind
subset so generated backends can name all catalog kinds. Bytes of generated modules stay
deterministic (stable ID order, canonical serialization).

**Rationale**: The SDK (`widgets`, component factories, `HostEventRegistry`, navigation) is
the documented Wasm-side contract; generated code that targets it inherits protocol
correctness. Reusing `emit()`'s mount/dispatch shape keeps the differential comparison with
the evaluator apples-to-apples.

**Alternatives considered**: string-template codegen independent of the SDK (rejected:
reimplements the guest runtime and risks protocol drift); NAPI-driven emit (rejected: the
emitter must stay pure-Rust per the offline requirement).

## R6. Fingerprints, caches, lifecycle

**Decision**: New `fingerprint.rs` defines `ArtifactFingerprint` hashing source contents,
dependency graph, language/IR version (`STUDIO_IR_VERSION` now 2), SDK/binding versions,
`rsvelte` revision (from `EngineFingerprint`), ASC version/options, and target ABI. Caches are
content-addressed by that key; dev edits mark Wasm stale without forcing ASC; the Wasm
snapshot refreshes once at the session boundary or on explicit build.

**Rationale**: No fingerprint concept exists yet in `studio-script`/`studio-package`; the
`rsvelte` `EngineFingerprint` pattern (schema-versioned namespaces) is the model to copy.
Deterministic inputs → byte-identical artifacts follows from hashing, not hoping.

**Alternatives considered**: mtime-based invalidation (rejected: feature 003 already proved
content beats mtime for generated-file hygiene).

## R7. Differential fixtures as the equivalence contract

**Decision**: New `studio_differential.rs` runs shared IR fixtures (literals, conditionals,
keyed iteration, state transitions, typed events) through three legs — `eval::dispatch`,
`assemblyscript::simulate_event`, and compiled-Wasm `studio_event` — and asserts identical
observable protocol message sequences in identical order. The existing `simulate_event` needs
no changes to join; the wasm leg reuses the starter-style harness.

**Rationale**: Equivalence proven by executed fixtures, not by code review of two backends;
this is the documented drift guard between Rust-dev and ASC output.

## R8. Dependency mechanics and offline rule

**Decision**: Vendor the five `rsvelte` crates under `vendor/rsvelte/` at the pinned rev with
`default-features = false` and no `projection` feature. Upstream's `oxc` git `[patch]` is not
copied (Studio never names `oxc` types). Dev sessions run fully
offline after install: parse, validate, swap, and ASC are all local (ASC already is; the new
adapter and evaluator introduce no network, threads, or runtimes).

**Rationale**: Vendoring a compiler workspace is premature before the adapter proves the API
stable enough to freeze; the git pin plus `Cargo.lock` gives reproducibility now, vendoring
later if upstream churn forces it.
