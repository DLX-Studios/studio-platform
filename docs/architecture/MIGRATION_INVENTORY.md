# Modular Platform Migration Inventory

Status: active migration control document. Update this file before moving a vertical slice.

## Repository boundary

`studio-runtime` is the public native runtime repository and migration target.
`/home/sir/Nova_Projects/studio-nova/apps/website` is explicitly out of scope: it is not a source,
reference, test fixture, or asset provider for this migration and must never be copied into this
repository. The rest of `studio-nova` is also not imported wholesale. A file may cross that
repository boundary only after an individual provenance, license, secret, and ownership review.

Use copy-then-adapt for approved source imports so the source repository remains intact. Record the
source path, upstream revision, license, destination, and changes in `THIRD_PARTY_NOTICES.md` before
committing a copied file. Never copy `.git`, build output, environment files, credentials, private
deployment configuration, or the excluded website.

## Current crate dependency graph

The arrows below mean "depends on". External and vendored dependencies are shown only where they
form an architectural boundary.

```text
studio-app
  -> studio-actions -> studio-security
  -> studio-components -> studio-protocol
                       -> studio-navigation
                       -> studio-security
                       -> studio-ui -> studio-protocol
                       -> vendor/gpui-component -> GPUI (pinned Wayland build)
  -> studio-navigation
  -> studio-package
  -> studio-protocol
  -> studio-security
  -> studio-ui
  -> studio-wasm -> studio-security
                 -> Wasmtime (no WASI)
  -> GPUI + gpui_platform (same pinned revision, Wayland feature only)

studio-cli -> studio-package
studio-script -> thiserror (compiler boundary; no runtime integration yet)
studio-testkit -> wat
studio-wasm (tests only) -> studio-testkit
```

There are no dependency cycles between Studio crates. `studio-protocol` is a leaf within the Studio
graph and remains the authoritative contract. `studio-app` is currently both composition root and
library host; separating those roles is a later compatibility-preserving slice.

## Ownership and target mapping

| Current owner | Authoritative responsibility | Intended logical destination | Compatibility rule |
|---|---|---|---|
| `studio-protocol` | protocol-v1 types, validation, catalog, event wire types | `studio-core/protocol` | keep crate name and public re-exports during migration |
| `studio-ui` | retained tree, stable identity, atomic mount/patch | `studio-core/ui-registry` | old crate remains a facade until consumers migrate |
| `studio-components` | native catalog, state, normalized events, accessibility policy | `studio-components/*` plus core component/event runtimes | split behind existing public API; no duplicate widgets |
| `studio-navigation` | routes, history stack, guards, transitions | `studio-navigation` and `studio-core/navigation-runtime` | host route ownership remains authoritative |
| `studio-wasm` | Wasmtime engine, ABI, policy, limits, lifecycle | `studio-host/wasm-runtime` | no WASI or new guest authority |
| `studio-security` | principals, capabilities, opaque secrets, redaction | `studio-security` and `studio-host/security` | security API changes require explicit threat review |
| `studio-package` | manifest, archive, signatures, integrity, trust | `studio-package` and `studio-host/package-loader` | verification occurs before execution |
| `studio-actions` | payments, confirmations, receipts, printing | `studio-host/capability-bridge` and `hardware` | secret/payment values remain host-owned |
| `studio-app` | native host, rendering composition, recovery and shutdown | `apps/studio-host` plus `crates/studio-host/rendering` | keep `studio-app` package/binary shim until release migration |
| `studio-cli` | build/generate/package commands | `apps/studio-cli` | keep `studio` binary name |
| `studio-script` | `.studio` compiler boundary and future typed IR | `studio-core/component-runtime` plus SDK tooling | no QuickJS; shared IR for dev and ASC |
| `studio-testkit` | deterministic Wasm and host fixtures | `studio-testkit` | tests move with owning module |

## Public API checkpoints

The public entry points are each crate's `src/lib.rs` re-exports plus the `studio` and
`studio-pack` binaries. Before a crate is split, capture its exported API with rustdoc JSON or an
equivalent checked fixture. During migration, old crate names re-export the new implementation;
callers must not be forced to switch paths in the same slice that moves ownership.

Security-sensitive public surfaces that require additional review are:

- `studio_security::{ActionGate, PluginPrincipal, SecretRegistry, OpaqueHandle}`.
- `studio_wasm::{SandboxEngine, ModulePolicy, PluginInstance, RuntimeBudgets, EmitBridge}`.
- `studio_package::{parse_manifest, inspect_archive, verify_bundle_signature, TrustStore}`.
- `studio_protocol::{decode_guest_message, validate_guest_message, UiNode, PatchBatch, HostEvent}`.
- `studio_ui::{UiRegistry, InstanceId, RetainedNode, PatchCommit}`.

## Source-of-truth and generated files

| Source of truth | Generated or derived output | Owner/check |
|---|---|---|
| `crates/studio-protocol/src/**` | `protocol/schemas/protocol-v1/*.schema.json` | `generate_schema` binary |
| Rust protocol inventory and checked schemas | `sdk/assemblyscript/assembly/generated/protocol.ts` | `scripts/generate-protocol.ts` |
| protocol fixtures and generator | `protocol/fixtures/**` | generator rejects stale files |
| `routes/*.studio` | `assembly/routes.generated.ts` | `studio-cli`; never hand-edit |
| AssemblyScript SDK sources | `sdk/assemblyscript/build/**` | Bun/ASC build output, not API source |
| vendored Longbridge source plus patch workflow | `vendor/gpui-component*` | upstream sync scripts and delta doc |

`protocol/` is the canonical contract tree. Generated files never initiate a move.

## Test ownership

- Protocol contract tests stay with `studio-protocol`; generated-artifact Bun tests validate the
  Rust-to-schema-to-AssemblyScript chain.
- Registry mount/patch tests stay with `studio-ui` and later move with `ui-registry`.
- Catalog, state, event, secret-input, update, accessibility, and reduced-motion tests move with
  their `studio-components` submodule.
- Route tree, history stack, guard, and transition tests stay with `studio-navigation`.
- Wasm ABI, module policy, fuel, epoch, memory, trap, and lifecycle tests move with `wasm-runtime`.
- Package archive, manifest, signature, and integrity tests stay with `studio-package`.
- Cross-boundary launch, recovery, compositor, payment, security, performance, and Wayland tests
  remain repository-level acceptance gates.
- Every component adapter additionally needs a non-POS gallery case before completion.

## Duplication decisions

| Area | Existing owners | Decision |
|---|---|---|
| component widgets | `gpui-component`, `studio-components` | Longbridge implements widgets; Studio supplies protocol adapters and lifecycle only |
| retained state | `studio-ui`, `studio-components::state`, upstream entities | registry owns node identity; component state registry owns GPUI entities; do not merge their responsibilities |
| routing | `studio-navigation`, `studio-app::router` | generic plugin routing stays in navigation; checkout/host-sensitive routes remain host-owned |
| events | protocol `UiEvent`, component dispatcher, SDK registry | one versioned envelope; host router owns registration and cleanup |
| animation/theme | vendored library and Studio policies | reuse upstream primitives; Studio owns semantic tokens and reduced-motion policy |
| sandbox/security | `studio-wasm`, `studio-security`, Oxide concepts | retain Wasmtime isolation; selectively implement reviewed concepts rather than adding Oxide |

## Upstream and license status

| Source | Status | License/provenance action |
|---|---|---|
| Longbridge `gpui-component` | vendored audited revision; primary implementation base | Apache-2.0 files and delta recorded in notices |
| Zed GPUI | pinned Git revision with Wayland-only features | distribution license review remains required |
| adabraka-ui | reference only; no local source imported | verify repository/revision/license before any code copy |
| gpui-nav | pattern source only; no local source imported | verify repository/revision/license before any code copy |
| gpui-router | pattern source only; no local source imported | verify repository/revision/license before any code copy |
| niklabh/oxide | audited concept source only | Apache-2.0 revision and no-copy result recorded |

## Vertical-slice gates

1. Complete the upstream component capability matrix with constructor, state entity, properties,
   composition, events, focus, accessibility, lifecycle, reduced motion, statefulness, and patch
   control status.
2. Extend protocol component/event contracts from that audited matrix.
3. Introduce the retained native state registry keyed by `(plugin_instance, node_id)`.
4. Add stateless adapters, then stateful adapters, without replacing upstream implementations.
5. Integrate bounded navigation presentation and matching.
6. Publish fluent AssemblyScript builders and typed event/reactivity bindings.
7. Add gallery slices before POS adoption, then perform full cleanup only after compatibility gates.

After every slice, run the validation sequence in `AGENTS.md` plus Bun generation, no-X11 feature
and binary checks, and the release build. A slice is not complete while old APIs lack shims or
generated output differs after regeneration.
