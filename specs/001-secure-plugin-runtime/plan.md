# Implementation Plan: Secure Native Plugin Runtime

**Branch**: `001-secure-plugin-runtime` | **Date**: 2026-08-03 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `specs/001-secure-plugin-runtime/spec.md`

## Summary

Build a Wayland-only Rust desktop runtime that verifies and isolates AssemblyScript WebAssembly
plugins, translates their versioned declarative UI protocol into retained GPUI components, and
mediates navigation, secrets, payments, and printing through host-owned services. Milestone one
delivers one signed POS plugin from catalog through receipt, using targeted atomic patches,
deterministic simulators, and a fresh-instance manual recovery path after guest termination.

The implementation begins with the native Wayland/no-X11 gate, then establishes authoritative
Rust contracts before adding the sandbox, renderer, SDK, packaging, security services, and POS
vertical slice. Every behavioral slice begins with a failing test and ends with the documented
focused and affected verification commands.

## Technical Context

**Language/Version**: Rust edition 2024 on `nightly-2026-03-04`; AssemblyScript 0.28.8;
TypeScript 5.9 for repository tooling

**Primary Dependencies**: GPUI and `gpui_platform` pinned to Zed revision
`1a246efd7e1b83ab568ec5e3e6c1a43a42e1abba`; an audited Studio fork/wrapper of
`gpui-component` revision `6c804fa7acaf0bce4659401821969da2b283dc30`; Wasmtime 36.0.8;
Serde/JSON Schema generation; Ed25519 and SHA-256 libraries selected during the packaging slice

**Storage**: No general guest persistence. Host reads an administrator-provisioned absolute local
bundle path and a provisioned trust store; secrets, checkout state, receipts, print previews, and
up to 10,000 terminal idempotency records are in-memory for the host-process lifetime.

**Testing**: Rust unit/contract/integration tests with `cargo test`; Studio-owned deterministic UI
model tests plus native headless Weston/Sway tests (the pinned GPUI `test-support` feature is
forbidden because it enables X11); Bun tests for generation and AssemblyScript behavior;
hostile-WASM fixtures; Cargo feature and `ldd` no-X11 checks; fuzz and benchmark smoke suites
during hardening

**Target Platform**: 64-bit Linux desktop/POS systems in a native Wayland session; no X11 or
XWayland build, linkage, runtime, or fallback path

**Project Type**: Native desktop application runtime with Rust workspace libraries, an
AssemblyScript SDK, generated contracts, and example plugins

**Performance Goals**: Warm launch to first usable native frame under 150 ms; 95% of normal user
interactions visible within 100 ms; single property patch p95 under 2 ms; 100-property batch p95
under 8 ms; normal transitions sustain 60 FPS; idle guests emit no repeated UI work

**Constraints**: No WASI, direct sockets, filesystem, raw devices, native handles, raw secrets,
guest frame callbacks, HTML/CSS, arbitrary drawing, or floating-point transaction amounts;
16 MiB guest memory; 15 million initialization fuel, 10 million event fuel, and a 50 ms epoch
deadline per call; 5,000 UI nodes; depth 64;
512 operations per patch; 16 pending actions; one visible plugin instance

**Scale/Scope**: One local operator, one visible plugin, a route stack capped at 32, a deterministic
local catalog, two simulator capabilities, and the complete protocol-v1 component catalog.
Acceptance baseline `STUDIO-BENCH-1` uses an Intel Processor N100 with 8 GiB RAM,
integrated Intel UHD graphics, NVMe storage, a 1920x1080@60 output, and native Weston. The report
records the exact device, kernel, Mesa, and Weston versions.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-checked after Phase 1 design.*

| Principle or gate | Pre-research | Post-design evidence |
| --- | --- | --- |
| I. Host Authority and Zero Trust | PASS | The data model and contracts keep rendering, navigation, secrets, capabilities, storage, and native resources in the host. The guest has one import and no WASI. |
| II. Test-First Evidence | PASS | The quickstart defines layered verification; task generation must attach a first failing test and verification command to every behavioral slice. |
| III. Versioned Contracts First | PASS | `studio-protocol` remains authoritative; Phase 1 defines ABI, bundle, UI, navigation, lifecycle, and action contracts before their implementations. |
| IV. Retained Native UI | PASS | Mount creates a retained registry; atomic patches mutate targeted state; GPUI owns layout, focus, accessibility, overlays, animation, and frame scheduling. |
| V. Wayland-Only Surface | PASS | Wayland is a first delivery gate. Default features are disabled and both feature-graph and binary-linkage checks reject X11/XCB. |
| VI. Small Traceable Slices | PASS | Delivery follows dependency-ordered vertical slices and the next stage will produce story/requirement-tagged tasks with narrow file scopes. |
| Security and resource constraints | PASS | Limits, closed schemas, signature validation order, opaque references, simulator-only actions, redaction, and fresh-instance recovery are defined in contracts and model transitions. |
| Workflow quality gates | PASS | Specify and clarify are complete; this plan supplies research/design. Tasks and analyze remain mandatory before implementation. |

No constitutional exception or complexity waiver is required.

## Architecture and Data Flow

```text
signed .studio bundle
        |
        v
studio-package -> verified PluginPrincipal -> studio-wasm (Wasmtime, no WASI)
                                               |
                              one emit import  |  queued HostEvent
                                               v
                                       studio-protocol
                                          /    |    \
                                         v     v     v
                                 studio-ui  navigation  actions/security
                                         \     |     /
                                          v    v    v
                                  Studio-owned GPUI shell
```

The runtime never calls back into a guest while handling `emit`. It copies and validates the
message, queues host work, returns, and delivers results through a later `studio_event` call.
Patch batches validate against a cloned/logical registry transaction and commit all-or-nothing.
Compositor loss cancels pending actions, revokes secrets, terminates guests, and exits cleanly.

## Delivery Order

1. **Wayland foundation gate**: expand the native gallery, audit dependency features, prove
   controlled startup failure, headless operation, accessibility primitives, animation, and no
   X11/XCB linkage.
2. **Protocol contracts**: complete closed Rust types, validators, stable errors, schemas,
   generated AssemblyScript bindings, golden fixtures, and compatibility checks.
3. **Sandbox runtime**: selectively adapt audited Oxide patterns, then add Wasmtime feature
   validation, checked memory transfer, fuel, epoch deadlines, store limits, lifecycle, queues,
   and adversarial modules.
4. **Retained UI**: create the validated node registry, Studio component wrappers, mount
   transactions, atomic patches, event ownership, focus/accessibility preservation, and overlays.
5. **AssemblyScript SDK**: implement typed widgets, state/derived/effect/batch semantics,
   binding disposal, cycle detection, event dispatch, and protocol generation.
6. **Navigation and animation**: implement the typed route tree, stack, parameters, lazy screens,
   guards, host clock transitions, reduced motion, and depth limits.
7. **Bundles and trust**: implement deterministic archives, closed manifests, streaming limits,
   RFC 8785 canonical digests, signature verification, trust-store lookup, exact import/export
   validation, explicit production-path selection, and dev mode.
8. **Security and actions**: implement principals, capability authorization, opaque references,
   trusted confirmation, idempotency, deterministic payment, structured receipts, and printing.
9. **POS vertical slice**: evolve protocol, SDK, native mapping, and example together until all
   five user stories run end-to-end.
10. **Hardening**: fuzz parsers and transitions, benchmark acceptance paths, audit dependencies
    and copied code, package the native app, and complete security review.

## Project Structure

### Documentation (this feature)

```text
specs/001-secure-plugin-runtime/
├── plan.md
├── research.md
├── data-model.md
├── quickstart.md
├── contracts/
│   ├── README.md
│   ├── actions-v1.md
│   ├── bundle-v1.md
│   └── host-guest-v1.md
└── tasks.md                 # created by speckit-tasks, not this stage
```

### Source Code (repository root)

```text
crates/
├── studio-app/              # Wayland shell, composition root, host-owned surfaces
├── studio-protocol/         # authoritative public types, validation, schema generation
├── studio-package/          # archives, manifests, digests, signatures, trust store
├── studio-wasm/             # Wasmtime engine, ABI, memory, budgets, lifecycle
├── studio-ui/               # retained registry, transactions, property validation
├── studio-components/       # Studio schema-to-GPUI/gpui-component wrappers
├── studio-navigation/       # route tree, stack, guards, transitions
├── studio-security/         # principals, handles, redaction, confirmation state
├── studio-actions/          # capability policy, payment/printer simulators
└── studio-testkit/          # fixtures, hostile modules, headless test helpers

sdk/assemblyscript/
├── assembly/                # generated protocol bindings, widgets, signals, helpers
└── tests/

examples/pos-checkout/
├── assembly/
├── assets/
└── tests/

protocol/                   # generated JSON Schemas and golden protocol fixtures
vendor/gpui-component/       # introduced only after Wayland/transitive audit passes
scripts/                     # generation, packaging, no-X11, test, benchmark utilities
tests/                       # repository integration and end-to-end acceptance tests
docs/                        # ADRs, security model, SDK guide, provenance, operations
```

**Structure Decision**: Use a Rust workspace split along security and ownership boundaries, with
one AssemblyScript SDK workspace and one reference plugin. Crates are introduced only when their
delivery slice starts; `studio-protocol` stays dependency-light and authoritative, while GPUI and
Wasmtime remain isolated from the SDK and contract layers.

## Verification Strategy

- Every runtime change follows Red-Green-Refactor and records the initial expected failure.
- Contract fixtures must round-trip in Rust and AssemblyScript and reject unknown fields/kinds.
- Security suites exercise invalid archives, signatures, pointers, encodings, imports, limits,
  cross-principal handles, expiry, reuse, amount substitution, and redaction.
- UI tests assert atomicity and preserved node/focus/scroll identity, not only screenshots.
- Platform CI builds with `DISPLAY` unset, runs smoke tests in headless Wayland, examines Cargo
  features, and inspects the linked release binary.
- Performance results record hardware/profile, warm-up, sample count, p50/p95, and regressions.
- Manual checks are limited to native behavior that cannot yet be asserted through GPUI/headless
  harnesses and are never substitutes for automatable security behavior.

## Complexity Tracking

No constitution violations require justification.
