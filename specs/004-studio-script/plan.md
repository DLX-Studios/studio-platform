# Implementation Plan: Studio Script and Embedded Development Host

**Branch**: `004-studio-script` | **Date**: 2026-09-03 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/004-studio-script/spec.md`

## Summary

Extend the existing `studio-script` compiler boundary into a full Svelte-compatible source
pipeline: a private `rsvelte` adapter (git-pinned facade for fingerprints, `rsvelte_core` parse
API for the template tree, never codegen) feeds Studio IR, which evolves from its current static
skeleton (v1) to the typed dynamic model from the design doc (v2: props/state/derived slots,
handlers, template nodes, typed expressions). A portable-subset validator owns new STUDIO3xx
diagnostics; a Rust evaluator projects IR onto the retained registry (`UiRegistry`) with
transactional, identity-preserving hot swap; an extended AssemblyScript backend emits
SDK-targeting modules for ASC; fingerprinted caches and a module graph govern the lifecycle; and
differential fixtures prove Rust-evaluator ≡ ASC/Wasm protocol equivalence.

## Technical Context

**Language/Version**: Rust nightly-2026-03-04 (pinned); AssemblyScript 0.28.20 via ASC; Bun
1.3.9 for toolchain orchestration

**Primary Dependencies**: `rsvelte` facade + `rsvelte_core` parse API (Cargo git dependency,
pinned rev `b64aed9c`, MIT, pure Rust, no Node/network; details in research.md R1); existing
`studio-protocol`, `studio-ui`, `studio-components`, `studio-package`, `studio-wasm`, ASC

**Storage**: N/A; parsed IR and generated artifacts are fingerprinted build caches, not persisted
state

**Testing**: cargo integration tests in `crates/studio-script/tests/` (parser/validator/IR
fixtures, evaluator, hot swap, lowering, differential) plus existing Bun e2e and the studio
workspace suites

**Target Platform**: Linux desktop/POS on native Wayland for the dev runtime; Wasm bundles run
in the existing Wasmtime sandbox for production

**Project Type**: Compiler + embedded development runtime inside the native desktop toolchain

**Performance Goals**: Dev swaps complete within the existing interaction budget on the
development validation host; clean reference builds stay under the existing toolchain budgets

**Constraints**: Closed protocol and catalog; no QuickJS anywhere; no `rsvelte` types leak past
`studio-script`; no new capabilities/secrets/network; portable subset enforced before either
backend; additive-only diagnostic and exit-code families

**Scale/Scope**: Reference component set (props/defaults, state, derived, conditionals, keyed
iteration, interpolation, typed events); three example projects' consumption via generated ASC;
local filesystems only

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

PASS. Host authority preserved (IR is data; the existing sandbox/ABI/trust machinery admits the
lowered Wasm unchanged). Test-first honored (malformed/determinism/invalidation/differential
fixtures precede or accompany each behavior). Contracts versioned (IR v2 bump, additive STUDIO3xx
codes, schema-versioned fixture goldens). Work is sliced by user story with an MVP (compile +
validate) before evaluator and lowering. No new network, storage, or capability paths; the
`rsvelte` dependency is a pure-Rust parser with no runtime services. Dev swaps stay within the
existing bounded resource model (atomic transactions, bounded templates).

## Project Structure

### Documentation (this feature)

```text
specs/004-studio-script/
├── plan.md              # This file
├── research.md          # Phase 0 output
├── data-model.md        # Phase 1 output
├── quickstart.md        # Phase 1 output
├── contracts/           # Phase 1 output (diagnostics, IR fixtures, CLI surface)
└── tasks.md             # Phase 2 output ($speckit-tasks command)
```

### Source Code (repository root)

```text
crates/studio-script/
├── src/
│   ├── lib.rs                 # compiler boundary (prepare/compile stay, plus compile_studio)
│   ├── rsvelte_adapter.rs     # the only module coupled to rsvelte (new)
│   ├── validate.rs            # portable-subset + catalog validation (new)
│   ├── types.rs               # StudioType system (new)
│   ├── ir/                    # v2 dynamic IR (extended from existing ir.rs)
│   ├── eval/                  # Rust expression/template evaluator (new)
│   ├── hot_swap.rs            # transactional identity-preserving replacement (new)
│   ├── graph.rs               # module graph + cycle detection (new)
│   ├── fingerprint.rs         # artifact cache keys (new)
│   └── lower/assemblyscript.rs# extended ASC backend (from existing emitter)
└── tests/
    ├── studio_ir_v2.rs        # golden IR fixtures
    ├── studio_validate.rs     # subset rejection coverage
    ├── studio_eval.rs         # Rust evaluator behavior
    ├── studio_hot_swap.rs     # swap contract coverage
    └── studio_differential.rs # Rust-dev vs ASC/Wasm equivalence
```

**Structure Decision**: All compiler/runtime work stays inside `studio-script` behind its existing
public boundary; the dev watcher and CLI build command consume it (watcher drives hot swap in
US2 follow-up; `studio build` routes `.studio` projects through lowering in US3). No new crates;
`rsvelte` is the only new dependency and is fenced to one adapter module.

## Complexity Tracking

> **Fill ONLY if Constitution Check has violations that must be justified**

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|-------------------------------------|
| Git dependency on pinned `rsvelte` rev | No Svelte-compatible Rust parser exists on crates.io; the alternative is writing a Svelte parser from scratch | A Studio-owned Svelte parser would be a second compiler project with no compatibility oracle |
