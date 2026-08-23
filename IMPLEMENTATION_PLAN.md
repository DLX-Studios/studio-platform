# Studio Runtime — Architecture and Implementation Plan

Status: approved for implementation
Repository: `/home/sir/Nova_Projects/studio-platform`
Initial milestone: Wayland-only native POS checkout using a sandboxed AssemblyScript plugin

## 1. Summary

Studio is a native Linux application runtime for sandboxed business plugins. Plugins are written in AssemblyScript, compiled to WebAssembly, and declare retained UI trees using Flutter-like primitives. The Rust host validates those trees, renders native GPUI widgets, and applies targeted property patches instead of rebuilding the complete interface.

The first complete vertical slice is a simulated POS checkout: load and verify a signed plugin bundle, render a catalog and cart, navigate through checkout, capture a PIN in host-owned memory, execute a deterministic simulated payment through an opaque handle, and display and print a simulated receipt.

Studio is a new standalone project. Existing Canvas and Studio OS repositories remain unchanged.

## 2. Fixed Decisions

- Host: Rust, GPUI, and a Studio-owned audited fork/wrapper of `gpui-component`.
- Toolchain: dated Rust nightly `nightly-2026-03-04`, required by the pinned GPUI revision.
- Delivery workflow: GitHub Spec Kit `v0.15.2` with native Codex skills; implementation tasks are executed test-first.
- Platform: general Linux desktop and POS hardware using Wayland only.
- Unsupported: X11 and XWayland, including silent fallback.
- Guest: AssemblyScript compiled to `wasm32-unknown-unknown`.
- Runtime: in-process Wasmtime without WASI.
- Rendering: retained host widget tree with atomic targeted patches.
- Schema: strict, versioned Studio protocol with Flutter-like component names.
- Navigation: host-managed typed route tree plus stack operations.
- Animation: host-scheduled transitions; no guest frame loop.
- Security: signed bundles, closed manifests, deny-by-default capabilities, opaque handles, fuel, epoch deadlines, and Wasmtime store resource limits.
- Milestone hardware: deterministic payment and printer simulators.
- Deferred: browser/web renderer, real hardware, arbitrary networking, and persistent guest storage.

## 3. Upstream Projects and Provenance

Research is pinned to:

- Oxide: `29cd89882465d6ebfe00af2ada6f89951581c580`
- gpui-component: `6c804fa7acaf0bce4659401821969da2b283dc30`
- adabraka-ui: `e158684b23d9cb043fed3989ca252212046dabca`
- gpui-nav: `fecccf8c0d641efc75152fa206bbb941fa990c70`
- gpui-router: `b8b4228d9a1cb2bb108432241bcb5d8e6784a035`

Usage:

- `gpui-component` is the native component foundation, isolated behind Studio-owned interfaces.
- `adabraka-ui` is a reference for animation, easing, layout, and component ergonomics only.
- `gpui-nav` and `gpui-router` are references for stack and route-tree behavior only.
- Oxide is an audited source-code donor, not the application foundation.

Any copied or substantially derived Apache-2.0 code receives file-level attribution and an entry in `THIRD_PARTY_NOTICES.md`. `docs/upstream/oxide-audit.md` records source paths, modifications, threat assumptions, and tests.

## 4. Workspace Layout

```text
studio-platform/
├── IMPLEMENTATION_PLAN.md
├── Cargo.toml
├── Cargo.lock
├── rust-toolchain.toml
├── package.json
├── bun.lock
├── THIRD_PARTY_NOTICES.md
├── crates/
│   ├── studio-app/
│   ├── studio-protocol/
│   ├── studio-package/
│   ├── studio-wasm/
│   ├── studio-ui/
│   ├── studio-components/
│   ├── studio-navigation/
│   ├── studio-security/
│   ├── studio-actions/
│   └── studio-testkit/
├── sdk/assemblyscript/
├── examples/pos-desktop/
├── protocol/
├── vendor/gpui-component/
├── docs/{architecture,security,sdk,upstream}/
└── scripts/
```

Rust types in `studio-protocol` are authoritative. JSON Schema and AssemblyScript bindings are generated from them and checked into the repository. Bun manages AssemblyScript dependencies, generation scripts, builds, and tests; Cargo manages the native workspace.

## 5. Wayland-Only Foundation Gate

Before the plugin runtime, build a minimal GPUI application containing text, a button, text input, scrolling, a popup, focus traversal, and one animation.

Acceptance requirements:

- GPUI default features that introduce X11 are disabled.
- The `gpui-component` fork uses the same restricted feature set.
- If upstream cannot compile without X11, patch the pinned sources to remove X11 compile paths.
- No runtime XWayland fallback.
- Absence of a Wayland compositor produces a concise controlled error.
- CI builds with `DISPLAY` unset.
- `ldd` fails CI if the release binary links `libX11`, `libxcb`, or another X11 client library.
- Interactive integration tests run under headless Weston or Sway.
- The pinned GPUI `test-support` feature is not enabled because it currently enables X11; Studio
  uses deterministic view-model tests plus the native headless-Wayland harness instead.

This is a hard gate before implementing the WASM-to-widget renderer.

## 6. Oxide Extraction and Hardening

Selectively port:

- The `WasmEngine`/`SandboxPolicy` separation.
- Background compilation and runtime-to-UI channel patterns.
- Checked `(ptr, len)` guest-memory helpers.
- Module lifecycle states: loading, running, trapped, and stopped.
- Streaming input-size enforcement.
- Permission request state-machine concepts.
- Identity-scoped permission concepts, translated from URL origins to signed plugin principals.

Do not port:

- Browser tabs, URLs, history, bookmarks, Forge, or remote navigation.
- Immediate-mode canvas rendering or `on_frame` callbacks.
- Rust guest SDK.
- Guest WebGPU, WebRTC, WebSocket, media, MIDI, file, clipboard, download, or child-module APIs.
- Optional manifests, the full linker, `sled`, or Oxide's resource defaults.

Studio adds controls absent from the audited Oxide implementation: `StoreLimits`, epoch interruption, strict WASM-feature validation, small message budgets, and a required signed manifest.

## 7. Bundle and Trust Format

A `.studio` bundle is a byte-deterministic ZIP using stored entries, lexicographic UTF-8 paths,
fixed `1980-01-01T00:00:00` timestamps, regular-file mode `0644`, and no extras or comments:

```text
manifest.json
module.wasm
assets/...
signature.ed25519
```

Manifest version 1:

```json
{
  "schemaVersion": 1,
  "id": "com.example.pos",
  "name": "Example POS",
  "version": "0.1.0",
  "publisher": { "id": "example", "keyId": "dev-example-1" },
  "entry": "module.wasm",
  "sdkVersion": "^0.1.0",
  "protocolVersion": 1,
  "capabilities": ["payment.simulate", "printer.simulate"],
  "limits": { "memoryMiB": 16, "eventFuel": 10000000 }
}
```

Validation order:

1. Limit archive input to 16 MiB.
2. Reject traversal, symlinks, duplicate paths, and decompression bombs.
3. Limit WASM to 8 MiB and milestone assets to 1 MiB total.
4. Parse a closed schema; unknown fields fail.
5. Build an RFC 8785 canonical JSON digest document from the complete manifest, module, and
   ordered assets.
6. Verify the raw 64-byte Ed25519 signature against the host trust store.
7. Validate identity, versions, and capabilities.
8. Validate WASM imports and exports.
9. Compile and instantiate only after all checks pass.

Developer mode requires `--dev`, accepts only an explicitly provided path, displays a persistent untrusted banner, and still enforces every sandbox and schema limit. Hardware actions remain simulators.

Production mode requires `--bundle` with an administrator-provisioned absolute path to one local
`.studio` regular file. File location grants no trust and every production verification remains
mandatory.

## 8. Principal and Capability Model

```rust
struct PluginPrincipal {
    plugin_id: PluginId,
    publisher_key_id: KeyId,
    bundle_digest: Sha256Digest,
    instance_id: InstanceId,
}
```

Every request is checked against the signed declaration, host policy, principal, operation, user-confirmation requirements, and resource constraints.

Milestone capabilities are exactly:

- `payment.simulate`
- `printer.simulate`

Unknown capabilities fail manifest validation. Undeclared requests return `capability_denied` without prompting.

## 9. WASM Runtime and ABI

Start with Wasmtime `36.0.8`, matching the audited Oxide revision. Do not create a WASI linker.

Guest exports are exactly these five entries; AssemblyScript runtime/user extras are rejected:

```text
memory
studio_alloc(len: i32) -> i32
studio_dealloc(ptr: i32, len: i32)
studio_init(ptr: i32, len: i32) -> i32
studio_event(ptr: i32, len: i32) -> i32
```

The only guest import is:

```text
studio_host.emit(ptr: i32, len: i32) -> i32
```

Communication uses UTF-8 JSON protocol-v1 envelopes. The envelope discriminant selects mount, patch, navigation, action, or log messages. Host responses arrive later through `studio_event`; the host never re-enters a guest during `emit`.

Limits:

- 16 MiB memory, one memory, one table.
- Disable threads, shared memory, memory64, multi-memory, unknown proposals, and unexpected imports.
- 15 million fuel for initialization and 10 million fuel per event.
- 50 ms epoch deadline per guest call.
- Store resource limiter applies to guest-created resources.
- Inbound event: 64 KiB.
- Initial UI tree: 1 MiB.
- Patch batch: 256 KiB, at most 512 operations.
- String property: 64 KiB.
- UI nodes: 5,000; tree depth: 64.
- Pending action requests: 16.

Malformed pointers, invalid UTF-8, excess resources, fuel exhaustion, epoch interruption, protocol violations, or traps terminate the plugin instance and show a host-owned error surface.

## 10. Public Protocol

```rust
enum GuestMessage {
    Mount(MountTree),
    Patch(PatchBatch),
    Navigate(NavigationCommand),
    Action(ActionRequest),
    Log(GuestLog),
}

enum HostEvent {
    Ui(UiEvent),
    Navigation(NavigationEvent),
    ActionResult(ActionResult),
    Lifecycle(LifecycleEvent),
}

enum PatchOp {
    UpdateProp { node_id: NodeId, property: PropertyName, value: PropValue },
    InsertChild { parent_id: NodeId, index: u32, node: UiNode },
    RemoveNode { node_id: NodeId },
    ReplaceNode { node_id: NodeId, node: UiNode },
}
```

Patch batches validate fully and apply atomically. Node IDs are stable, unique per plugin instance, and no longer than 128 bytes.

## 11. UI Schema

Initial nodes:

- Layout: `Box`, `Container` alias, `Column`, `Row`, `Stack`, `Grid`, `ScrollView`, `ListView`, `Spacer`, `Divider`.
- Display: `Text`, `Icon`, `Image`, `Card`, `Badge`, `ProgressIndicator`.
- Interaction: `Button`, `IconButton`, `Checkbox`, `Switch`, `Slider`, `Select`, `TextInput`, `SecretInput`.
- Overlay: `Dialog`, `BottomSheet`, `Toast`, `Tooltip`.

`Box` maps to GPUI's flexible `div()` primitive. Each node and property uses a closed schema. Styling is typed: semantic colors, spacing scale, constraints, flex, alignment, border, radius, shadow, opacity, typography role, visibility, enabled state, and approved transitions.

Plugins cannot provide HTML, CSS strings, shaders, raw drawing commands, or native component class names.

## 12. Retained Rendering

Mount validation builds a `NodeRegistry<NodeId, NativeNode>`, maps schema nodes to Studio component wrappers, registers events, and commits one GPUI tree.

Patch handling validates every operation, constructs a mutation transaction, commits atomically, and invalidates only affected entities or layout branches. Common text, enabled, value, opacity, color, and progress updates must not rebuild unrelated ancestors.

## 13. AssemblyScript SDK

```ts
$state<T>(initial: T): State<T>
$derived<T>(compute: () => T): Derived<T>
$effect(run: () => void): EffectHandle
batch(run: () => void): void
mount(root: Widget): void
```

Reads register dependencies; writes mark dependents dirty; changes flush after the current handler; batches coalesce writes; derived values memoize; node removal disposes effects; cycles and excess iterations fail predictably. Initial mounting emits one tree and subsequent bindings emit property patches.

Money uses integer minor units:

```ts
class Money {
  currency: string;
  minor: i64;
}
```

Floating point values never represent payment amounts.

## 14. Navigation and Animation

Navigation supports nested, index, named, parameterized, and not-found routes; `push`, `replace`, `pop`, `popTo`, and `reset`; lazy screens; route-local state; guards; and a maximum stack depth of 32.

Initial POS routes:

```text
/
├── /catalog
├── /cart
├── /checkout
│   └── /checkout/payment
└── /receipt/:receiptId
```

Commands are validated requests to the host. A guard has 50 ms to respond; timeout blocks navigation and displays a warning.

Defaults:

- Push: 180 ms horizontal slide and fade.
- Pop: reverse push.
- Replace: 120 ms cross-fade.
- Reduced motion: zero duration.

The host monotonic clock and GPUI frame scheduling drive animations. Guests receive no animation-frame callback.

## 15. Opaque Secret Handles

`SecretInput` is completely host-owned. The guest receives only a reference containing handle, kind, and expiration time.

- Handles contain 256 bits of cryptographic randomness.
- Raw secrets stay in host memory.
- Handles bind to principal, instance, secret kind, intended action, and checkout session.
- Default TTL is 120 seconds.
- PIN/payment handles are single-use.
- Secrets and handles never enter logs, crash reports, storage, receipts, or snapshots.
- Buffers clear on consume, expiry, instance shutdown, and host shutdown.
- Wrong-principal, wrong-action, wrong-session, expired, and reused handles fail.

A host-owned confirmation surface shows publisher, amount, currency, and simulator status. Confirmation binds the exact amount so later guest state cannot modify it.

## 16. Action Protocol and Simulators

```rust
struct ActionRequest {
    request_id: RequestId,
    capability: CapabilityId,
    operation: String,
    payload: JsonValue,
}

enum ActionResult {
    Success { request_id: RequestId, payload: JsonValue },
    Failure { request_id: RequestId, code: ActionErrorCode, message: String, retryable: bool },
}
```

Payment `charge` requires checkout session, integer amount, currency, auth handle, and idempotency key.

Deterministic results by amount suffix:

- `00`: approved.
- `01`: declined.
- `02`: timeout.
- `03`: terminal unavailable.

Terminal idempotency records remain in memory until host process exit. Repeated keys return the
original result. The registry holds at most 10,000 terminal records; when full, new unique keys
fail without evicting retained records, while retained-key replay continues. No network is used.

The printer simulator accepts a structured receipt, records an in-memory print job, and displays a host preview. Guests cannot submit raw ESC/POS bytes.

## 17. POS Reference Plugin

The example covers catalog grid and search, cart quantities, subtotal/discount/tax/total derivations, patch batching, checkout navigation, host PIN entry, confirmation, every simulator outcome, retry, receipt, printer preview, and a back guard during payment.

It is both the primary integration fixture and SDK tutorial.

## 18. Delivery Phases

1. **Phase 0 — Foundation:** workspace, implementation plan, ADRs, threat model, provenance, dependency pins, and CI skeleton.
2. **Phase 1 — Wayland gate:** minimal native gallery, restricted features, no-X11 linkage check, and headless compositor test.
3. **Phase 2 — Protocol:** Rust types, schemas, generated AssemblyScript bindings, fixtures, and compatibility tests.
4. **Phase 3 — Sandbox:** audited Oxide extraction, hardened Wasmtime engine, ABI, lifecycle, and hostile module tests.
5. **Phase 4 — Retained UI:** validation, node registry, component mapping, events, patches, focus, overlays, and accessibility.
6. **Phase 5 — SDK:** widgets, signals, bindings, batching, navigation and action helpers, tests, and documentation.
7. **Phase 6 — Navigation:** route matcher, stack, lazy screens, guards, transitions, and reduced motion.
8. **Phase 7 — Packaging:** deterministic bundles, schema checks, signatures, trust store, and dev mode.
9. **Phase 8 — Security/actions:** principals, capability decisions, opaque handles, confirmations, and simulators.
10. **Phase 9 — POS slice:** complete catalog-to-receipt example.
11. **Phase 10 — Hardening:** fuzzing, benchmarks, dependency audit, packaging, and security review.

## 19. Test and Acceptance Matrix

### Platform

- Build and run with `DISPLAY` unset.
- Run under headless Wayland.
- No X11/XCB dynamic linkage.
- Controlled error without a compositor.

### Package security

- Valid bundle succeeds; modified manifest, module, asset, or signature fails.
- Unknown fields, traversal, duplicates, symlinks, excess sizes, and decompression bombs fail.
- Unsigned bundles require explicit developer mode.

### WASM isolation

- WASI and unknown imports fail.
- Infinite loops stop through fuel or epoch deadline.
- Memory beyond 16 MiB fails.
- Bad pointers, overflow, UTF-8, messages, trees, and node floods fail safely.
- Traps do not crash or freeze the host.
- Principals cannot cross-use handles or results.

### UI and reactivity

- All nodes render in light and dark themes.
- Invalid nodes, properties, and duplicate IDs fail.
- Patch batches are atomic.
- Property patches avoid unrelated rebuilds.
- Focus survives valid updates.
- State invalidates only dependents; batches coalesce; effects dispose; cycles terminate.

### Navigation and actions

- All route forms and stack operations work, including depth and guard limits.
- Lazy routes instantiate only when matched.
- Raw PIN bytes never enter guest memory.
- Handles expire, are single-use, and are context-bound.
- Confirmation binds amount and currency.
- Simulator results and idempotency are deterministic.
- Printer accepts structured receipts only.

### Performance targets

- Warm initialization to first native frame: under 150 ms.
- Single property patch: p95 under 2 ms.
- 100-property batch: p95 under 8 ms.
- Normal POS transitions maintain 60 FPS on the documented baseline machine.
- Idle plugins run no guest frame loop.
- Compilation and traps do not block host responsiveness.

Performance acceptance uses `STUDIO-BENCH-1`: an Intel Processor N100 device with
8 GiB RAM, integrated Intel UHD graphics, NVMe storage, 1920x1080@60 output, and native Weston.
Record exact device/kernel/Mesa/Weston versions. Warm launches use 5 warm-ups and 30 samples;
catalog/cart/navigation operations use 10 warm-ups and 100 samples per operation, measured from
host event receipt to frame presentation with monotonic time.

## 20. Milestone-One Completion Criteria

- The host is demonstrably Wayland-only.
- A signed AssemblyScript POS bundle renders through native GPUI widgets.
- Fine-grained state emits and applies targeted patches.
- Native navigation covers the complete checkout flow.
- Raw PIN data never enters guest memory.
- Payments and printing require declared, validated capabilities.
- Hostile modules are contained by memory, fuel, epoch, ABI, and message limits.
- Oxide-derived code is audited, attributed, and Studio-tested.
- Rust, AssemblyScript, schema, integration, fuzz-smoke, security, performance-smoke, and no-X11 CI checks pass.

## 21. Explicitly Deferred

- Web, macOS, and Windows hosts.
- Real payment providers, printers, and terminals.
- Generic outbound HTTP and persistent plugin storage.
- Dynamic third-party native components.
- Canvas, raw GPU access, shaders, WebRTC, media, and arbitrary filesystem access.
- Multi-plugin composition on one screen.
- Hot protocol upgrades without rebuilding plugins.
- Remote bundle discovery and a marketplace.
