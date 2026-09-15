# Phase 0 Research: Secure Native Plugin Runtime

## Research Scope

This research resolves the technical choices required by the feature specification and the
Studio constitution. The existing architecture plan and pinned upstream revisions are treated as
approved constraints. Primary upstream documentation was checked for GPUI's retained/declarative
model, `gpui-component` coverage, and Wasmtime resource-interruption behavior.

## Decision 1: Native UI Foundation

**Decision**: Use pinned GPUI as the rendering/event foundation and introduce the pinned
`gpui-component` revision only behind `studio-components` wrappers after the Wayland dependency
audit passes. Map protocol `Box` and `Container` to GPUI's general-purpose `div()` layout element.

**Rationale**: GPUI supplies entities, declarative views, low-level elements, actions, platform
services, an integrated executor, and test contexts. Its `div()` already provides the flexible
container primitive sought from web/Svelte-style UI. `gpui-component` provides a broad maintained
component catalog, themes, virtualized controls, overlays, and accessibility-oriented native
controls without exposing its API as the plugin contract.

**Alternatives considered**: Building every widget directly on GPUI would increase milestone
risk; using adabraka-ui as a dependency would couple Studio to a less suitable public surface;
exposing `gpui-component` types directly would make the plugin protocol inherit upstream churn.

**Sources**: [GPUI README](https://github.com/zed-industries/zed/blob/main/crates/gpui/README.md),
[gpui-component](https://github.com/longbridge/gpui-component)

## Decision 2: Wayland-Only Enforcement

**Decision**: Disable GPUI default features, enable only the Wayland platform feature, audit the
complete feature graph before importing `gpui-component`, and enforce the constraint through
source configuration, `cargo tree`, release linkage inspection, native-session startup checks,
and a headless Wayland run.

**Rationale**: A runtime check alone cannot prevent accidental X11 linkage or XWayland fallback.
The feature graph and final binary are both acceptance artifacts. The current minimal host has
already demonstrated the restricted GPUI feature selection.

**Alternatives considered**: Supporting both Linux backends violates the feature; environment-only
checks are insufficient; forking GPUI immediately adds maintenance before proving it is needed.

The pinned GPUI revision's `test-support` feature also enables its `x11` feature. Studio therefore
does not use `gpui::TestAppContext` in milestone-one CI. Deterministic Studio view-model tests cover
state and accessibility contracts, while the actual GPUI view is exercised under native headless
Wayland. A future audited GPUI fork may split test support from X11, but that is not required for
the foundation gate.

## Decision 3: Component Fork Policy

**Decision**: Treat `gpui-component` as an audited internal fork/wrapper, pin its revision, retain
only milestone components, record deltas, and update it together with GPUI behind wrapper tests.

**Rationale**: GPUI is pre-1.0 and changes frequently, while `gpui-component` brings a large
transitive surface. A wrapper prevents those APIs and changes from crossing the versioned plugin
boundary and permits removal of non-Wayland features.

**Alternatives considered**: A floating Git dependency is not reproducible; vendoring the entire
library before audit increases attack surface; copying individual widgets loses upstream history.

## Decision 4: Navigation

**Decision**: Implement a Studio-owned typed route tree and bounded stack in
`studio-navigation`, informed by `gpui-nav` and `gpui-router`, rather than adopting either as the
public or security-sensitive router.

**Rationale**: Route validation, guards, instance ownership, lazy screen creation, pending-payment
rules, and stable error events are part of the host/guest contract. Owning the small router keeps
those rules atomic with the retained UI registry and supports Flutter/Svelte-like ergonomics in
the SDK without importing library-specific behavior.

**Alternatives considered**: Directly adopting `gpui-nav` gives stack behavior but not the full
typed contract; directly adopting `gpui-router` couples validation and lifecycle to an upstream
API; guest-managed navigation weakens host authority.

## Decision 5: Animation

**Decision**: Use the host monotonic clock and GPUI frame scheduling for approved property and
route transitions. Plugins request typed transition policies but receive no frame callback.

**Rationale**: GPUI supports scheduled rendering and upstream examples include animation. Host
ownership makes reduced motion authoritative, bounds work, and prevents idle or malicious guests
from driving a render loop. Adabraka-ui remains a design/easing reference only.

**Alternatives considered**: A guest `requestAnimationFrame`-style API violates retained-idle
goals; implementing a separate animation runtime duplicates GPUI scheduling; no animation would
unnecessarily reduce UI quality.

## Decision 6: Wasmtime Sandbox

**Decision**: Use in-process Wasmtime 36.0.8 without WASI. Validate module features and imports
before instantiation, apply `StoreLimits`, allow 15 million deterministic initialization fuel, reset
10 million deterministic fuel for each event call, and set a trapping epoch deadline equivalent to
50 ms of host scheduling time.

**Rationale**: Fuel gives deterministic instruction budgeting, while epochs bound wall-clock
execution with lower incremental overhead. Wasmtime documents that both mechanisms trap guest
execution and that an epoch deadline must be explicitly set. Store limits cover guest-created
memory/table resources that host allocation wrappers alone cannot constrain.

**Alternatives considered**: Fuel alone does not express elapsed-time responsiveness; epochs alone
are nondeterministic and weaker for reproducible adversarial tests; subprocess isolation is a
future defense-in-depth option but adds IPC and lifecycle complexity beyond milestone one.

**Sources**: [Wasmtime interruption guide](https://docs.wasmtime.dev/examples-interrupting-wasm.html),
[Wasmtime Store API](https://docs.wasmtime.dev/api/wasmtime/struct.Store.html)

## Decision 7: Oxide Reuse

**Decision**: Keep Studio as its own repository and selectively adapt Oxide's engine/policy split,
checked guest-memory helpers, lifecycle concepts, background compilation channels, and
identity-scoped permission ideas. Record every copied or derived item in the extraction ledger.

**Rationale**: Oxide contains useful sandbox patterns, but its browser shell, broad capabilities,
Rust guest SDK, immediate-mode canvas, and optional manifest are incompatible with Studio's trust
boundary. Selective extraction preserves provenance without inheriting excluded subsystems.

**Alternatives considered**: Forking Oxide as the application base creates extensive removal work
and accidental capability risk; reimplementing every low-level pattern discards useful audited
work; Extism adds an abstraction that does not remove the need for Studio-specific validation.

## Decision 8: Host/Guest ABI and Encoding

**Decision**: Require exactly five exports—`memory` plus four named Studio functions—and one
`studio_host.emit(ptr,len)` import, with no additional compiler/runtime exports. Communication uses
copied UTF-8 JSON protocol-v1 envelopes. The host does not re-enter the guest from `emit`;
responses are queued and delivered through a later `studio_event` call.

**Rationale**: The small ABI is easy to audit from AssemblyScript, supports closed versioned
messages, and avoids re-entrancy hazards. JSON favors debuggability for milestone one; strict byte
budgets and generated bindings control its overhead.

**Alternatives considered**: WIT/component-model adoption is promising but introduces toolchain
and AssemblyScript uncertainty; MessagePack/FlatBuffers reduce bytes but complicate first-release
debugging and generation; multiple specialized imports enlarge the attack surface.

## Decision 9: Contract Authority and Generation

**Decision**: Rust types in `studio-protocol` are authoritative. Generate checked-in JSON Schemas,
AssemblyScript bindings, fixtures, and documentation and fail CI when regeneration changes files.

**Rationale**: One source prevents host/SDK drift, while checked-in outputs make reviews and
consumer builds reproducible. Closed enums and unknown-field rejection are part of the security
boundary.

**Alternatives considered**: Hand-maintained parallel types will drift; schema-first generation
would make Rust validation secondary; permissive versioning hides incompatible proposals.

## Decision 10: Bundle and Trust Format

**Decision**: Use a byte-deterministic stored ZIP `.studio` archive with a closed v1 manifest,
module, bounded assets, and a raw 64-byte Ed25519 signature. Sign an RFC 8785 canonical JSON
document containing the manifest and ordered module/asset digests. Validate streaming sizes and
paths before reading content or compilation, then identity, versions, capabilities, exact imports,
and exact exports.

**Rationale**: Deterministic ordering and canonical inputs make signatures reproducible. Ed25519
is widely implemented and keeps keys/signatures compact. A strict validation order rejects cheap
structural attacks before expensive compilation.

**Alternatives considered**: Tar lacks ubiquitous single-file desktop tooling; signing raw ZIP
bytes makes producer metadata significant; unsigned production bundles violate publisher trust;
a remote marketplace is explicitly deferred.

## Decision 11: Secrets and Sensitive Actions

**Decision**: Store secret bytes only in host memory behind 256-bit random references scoped to
publisher, plugin, bundle, instance, purpose, and checkout session. PIN references expire after
120 seconds, are single-use, and clear on consume, expiry, plugin stop, compositor loss, or host
shutdown. Bind payment confirmation to exact integer minor units and currency.

**Rationale**: A handle is useful only when every resolution dimension is validated and the host
owns confirmation. Binding the confirmed amount prevents time-of-check/time-of-use substitution.

**Alternatives considered**: Passing encrypted PIN bytes to the guest still exposes reusable
ciphertext and key/oracle risks; reusable handles increase replay risk; plugin-owned confirmation
can misrepresent merchant or amount.

## Decision 12: Action Simulators and Milestone Storage

**Decision**: Expose only `payment.simulate` and `printer.simulate`. Keep receipts, print previews,
and up to 10,000 terminal idempotency results in host memory. Idempotency records live until host
process exit; at capacity, new unique payments fail while retained-key replays remain available.
Use deterministic payment outcomes and structured receipt input; perform no network or device I/O.

**Rationale**: This proves capability mediation, confirmation, idempotency, and receipt integrity
without introducing provider compliance, hardware protocols, or durable secret-bearing records.

**Alternatives considered**: Real provider/hardware integration is too broad for the trust-boundary
milestone; generic HTTP or printer byte streams violate deny-by-default capabilities.

## Decision 13: Failure and Platform Recovery

**Decision**: A trap or resource violation terminates the instance and displays a host-owned
surface. Recovery is an explicit manual restart into a new instance identity with no restored
guest state. Wayland compositor loss cancels pending actions, revokes secrets, terminates all
plugins, and exits without automatic session restoration.

**Rationale**: Fresh manual restart avoids corrupted state and restart loops. Continuing without
a compositor cannot safely present trusted confirmation or recovery UI.

**Alternatives considered**: Automatic restart can loop; state restoration may reintroduce the
fault or secret-adjacent state; compositor reconnection adds complex session semantics not needed
in milestone one.

## Decision 14: Test and Benchmark Baseline

**Decision**: Use Red-Green-Refactor at every behavioral boundary. Measure acceptance performance
on `STUDIO-BENCH-1`: an Intel Processor N100 device with 8 GiB RAM, integrated Intel
UHD graphics, NVMe storage, 1920x1080@60 output, and native Weston. Record exact device, kernel,
Mesa, and Weston versions. Warm-launch results use 5 warm-ups followed by 30 launches. Interaction
results use the fixed catalog/cart/navigation corpus, 10 warm-ups, and 100 samples per operation;
latency runs from host event receipt to presentation of the resulting frame using monotonic time.

**Rationale**: A named minimum profile makes timing criteria reproducible while allowing faster
developer machines. Security validation remains hardware-independent and must not be waived when
performance infrastructure is unavailable.

**Alternatives considered**: CI-only virtual GPU timings are unstable; an unspecified “typical
machine” is not measurable; premium discrete-GPU hardware would hide POS-class bottlenecks.

## Resolution Status

All technical-context unknowns are resolved, and no constitutional gate requires an exception.
