# Implementation Roadmap

The architectural definition of each phase is in the root `IMPLEMENTATION_PLAN.md`. This file records implementation status only.

## Phase 0 — Foundation

- [x] Create the standalone workspace.
- [x] Save the approved implementation plan.
- [x] Pin Rust, GPUI, Bun, AssemblyScript, and upstream research revisions.
- [x] Record architectural boundaries and the initial threat model.
- [x] Add Rust and Bun validation commands.
- [x] Add the initial CI workflow.
- [x] Install GitHub Spec Kit `v0.15.2` with native Codex skills.
- [x] Ratify the Studio constitution with test-first development as a non-negotiable principle.
- [x] Baseline the approved architecture as the first Spec Kit feature.

## Phase 1 — Wayland gate

- [x] Compile GPUI with only its Wayland feature.
- [x] Reject startup without a native Wayland endpoint.
- [x] Check the Cargo feature graph for X11 dependencies.
- [x] Check the linked binary for X11/XCB libraries.
- [x] Smoke-test the native event loop on Wayland.
- [x] Import the minimal `gpui-component` fork with X11 disabled.
- [x] Build the complete component/focus/animation gallery.
- [x] Automate a nested headless-compositor integration test.

## Phase 2 — Protocol

- [x] Establish version-one guest and host message types.
- [x] Add closed JSON decoding and initial structural limits.
- [x] Test unknown fields, duplicate IDs, operation floods, and byte limits.
- [x] Define closed property types for every component.
- [x] Generate JSON Schema and AssemblyScript bindings.
- [x] Add cross-language golden fixtures.

## Later phases

- [x] Phase 3 — Wasmtime sandbox and ABI.
- [x] Phase 4 — Retained native UI runtime.
- [x] Phase 5 — AssemblyScript SDK and signals.
- [x] Phase 6 — Navigation and animation.
- [x] Phase 7 — Packaging and trust.
- [x] Phase 8 — Opaque handles and action simulators.
- [x] Phase 9 — Complete POS vertical slice.
- [x] Phase 10 implementation — fuzzing, benchmarks, packaging, and security review.
- [ ] Milestone-one release — human accessibility/legal sign-off; low-power hardware certification
  remains deferred.

## Starter quickstart release check

- [x] The starter counter/derived-total plugin builds, packages, mounts, and emits a targeted patch.
- [x] `scripts/test-starter-quickstart.sh` enforces the documented ten-minute ceiling.
- Latest local validation: 4 seconds on 2026-08-04.

---

## Studio Feature Roadmap — detailed

This section merges the former `planning/FEATURE_ROADMAP.md` (now removed — `docs/` is the single source). The project advances one Spec Kit feature at a time.

### 001 — Secure Plugin Runtime — ✅ Complete (347643a)

Finished and converged as `specs/001-secure-plugin-runtime/` — T104 host-owned routing in the live GPUI window is wired and proven. Baseline committed as `347643a`.

Completion included host-owned route state in `studio-app`, guest navigation commands, catalog/cart/checkout/receipt/recovery/print-preview transitions, guarded payment exit/retry, keyboard/native-shell acceptance, and all `001` tests passing.

### 002 — Unified Native Component Platform — ✅ Implementation complete

Finished and converged as `specs/002-component-platform/` — catalog foundation, display/forms/overlays/navigation/data/SDK batches and polish are implemented. All 36 tasks (T001–T036) are checked. Rust formatting, Clippy, and the full Rust workspace suite pass; the current component-platform validation report records the JavaScript install limitation on the VFAT/exFAT checkout separately from implementation status.

The feature should first establish its protocol/catalog foundation, then implement the component batches in dependency order. Navigation is a later user story within this feature, not a reason to bypass the foundation.

- Scaffold and application shell components.
- AppBar, Sidebar, NavigationBar, NavigationRail, and Drawer.
- Tabs, Breadcrumb, Stepper, and Pagination.
- Typed selection and navigation events.
- Keyboard, focus, accessibility, and reduced-motion behavior.
- A multi-screen demonstration with route-local state restoration.

Completion requires the feature’s red-green-refactor tasks, generated-artifact checks, integration coverage, accessibility checks, quickstart validation, and full regression suite to pass.

### 003 — Studio Toolchain and Development Workflow — ✅ Complete

Finished as `specs/003-studio-toolchain/` — `studio` owns a phased (validate/compile/package)
build pipeline with stable diagnostic codes and documented exit codes, atomic byte-identical
bundles through `studio_package::pack_bundle`, a debounced single-flight content-hash watcher
that excludes generator outputs and cancels cleanly, and toolchain-owned packaging with
`scripts/build-example.ts` delegating as a shim. Clean/failed/repeated builds, watcher
semantics, and shim parity are covered by integration tests; see the feature validation report.

Deferred to features 004/005: in-process dev presentation (studio-host/shell/renderer
composition); dev-mode currently launches the runtime host in development mode.

### 004 — Studio Script and Embedded Development Host — ✅ Complete

Finished as `specs/004-studio-script/` — the fenced `rsvelte` frontend (vendored at the pinned
rev) feeds versioned typed IR with portable-subset validation; the Rust evaluator projects IR
with transactional identity-preserving hot swap; the extended AssemblyScript backend emits
standalone modules proven behaviorally identical to the evaluator; module graphs, fingerprints,
and CLI wiring close the lifecycle. See [STUDIO_SCRIPT_TRANSFORM_PIPELINE.md](STUDIO_SCRIPT_TRANSFORM_PIPELINE.md) for the Rust + `rsvelte` pipeline: parse source into a Svelte AST, validate and lower it into typed Studio IR, interpret that IR in the Rust development runtime for identity-preserving HMR, and lower the same IR to AssemblyScript for production ASC → Wasm builds. QuickJS is not part of this architecture.

Svelte-like authoring, but tags are Studio components:

```svelte
<script lang="ts">
  let { name, price, available = true } = $props();
  let quantity = $state(1);
  function addToOrder() { emit("add-to-order", { productId: name, quantity }); }
</script>
<Card id="product-card" padding={12}>
  <Text id="product-name" typographyRole="label">{name}</Text>
  <Text id="product-price">{formatMoney(price)}</Text>
  <Button id="add-button" disabled={!available} onclick={addToOrder}>
    {available ? "Add to cart" : "Unavailable"}
  </Button>
</Card>
```

Conventions: `app.studio.ts`, `components/*.studio`, `routes/*.studio`; virtual modules are reserved for generated/host-provided contracts such as `@studio/generated/routes` and the ASC bootstrap. Development hot-swaps typed Studio IR without rebuilding Wasm after every edit. Both `studio` and `studio-app` compose `studio-host`/`studio-shell`/`studio-renderer`.

### 005 — Runtime Reload and Preview Protocol — ✅ Complete

Finished as `specs/005-reload-protocol/`. Define CLI-to-runtime reload protocol, atomic validation/instantiation, window-preserving swap, safe disposal, preserved/reset state, failed-reload recovery, and e2e edit→rebuild→reload tests. `refresh studio-app window` is placeholder.

### 006 — File-Based Routing — ✅ Complete

After navigation contract + toolchain stable. Define `routes/` static/nested/param/not-found, metadata, deterministic registry (not just constants), reject duplicates/ambiguous/malformed, connect to resolver/SDK, dev+prod coverage. Current scanner only emits `declaredRoutes`.

### 007 — Asset Imports and Lucide Icons — ✅ Complete

After toolchain + route graph. Collect only referenced `lucide-static` assets via typed import graph (not regex), deterministic bundle without hand-editing `manifest.json`, user overrides, missing-icon diagnostics, license, bundle-size tests.

### 008 — Studio Script Developer Experience — ✅ Complete

Create after the core Studio Script compiler, typed IR, Rust development runtime, ASC lowering, reload protocol, routing, and asset graph are stable. This feature improves authoring ergonomics by composing the Rust `rsvelte` crates; it must not delay or expand the immediate Feature 004 compiler/runtime work.

Architecture: everything runs in Rust. `studio-language-server` composes `rsvelte` parsing, formatting, linting, and projection (the `rsvelte_fmt`, `rsvelte_lint`, and `rsvelte_projection` Rust crates at the pinned rev, vendored on demand exactly like the 004 frontend) with Studio-owned intelligence. The VS Code extension is a thin client only: it registers `.studio`, launches the Rust server binary over stdio, supplies syntax highlighting, and exposes format/check/build commands. It contains no compiler logic, mirroring the rust-analyzer shape. No JavaScript or TypeScript toolchain package is required: `svelte2tsx`, `svelte-check`, `oxlint-plugin`, and `vite-plugin-svelte` serve JavaScript-ecosystem workflows our authors never touch, and TypeScript acceptance cannot prove IR validity or AssemblyScript portability anyway — the Rust validator remains the sole authority. A browser preview, if ever scheduled, is a separate feature, not part of this one.

Planned integrations:

- Use the `rsvelte_fmt` Rust crate as the formatting foundation, with `.studio` recognition and Studio-specific formatting fixtures.
- Use the `rsvelte_lint` Rust crate for Svelte-language diagnostics and add Studio rules for catalog components, typed props/events, portable-subset restrictions, security boundaries, routes, and assets.
- Use the `rsvelte_projection` Rust crate for source-to-structure mappings (component/prop/event positions, rename and reference search across `.studio` modules, routes, and assets), keeping the faithful projection path authoritative.
- Extend `studio-language-server` with Studio-owned completion, hover, definition, rename, references, diagnostics, component prop/event intelligence, and route/asset navigation.
- Provide the thin Studio VS Code client described above, showing route, asset, component, and HMR status without owning compiler state, and degrading to syntax highlighting plus local commands if optional language features fail.

Tooling crates must be pinned and hidden behind Studio-owned commands or adapters. Compatibility fixtures should cover `.studio` parsing, formatting stability and idempotence, diagnostics, projection mappings, catalog completion, and source locations before an upstream update is accepted.

Completion requires editor smoke tests, deterministic formatting tests, cross-tool diagnostic fixtures, projection mapping tests, LSP protocol tests, and proof that editor tooling failures cannot alter compiler, IR, or release-build semantics.

## Required Spec Kit workflow

Every feature follows: `speckit-specify` → `speckit-clarify` → `speckit-plan` → `speckit-tasks` → `speckit-implement` (red-green) → `speckit-analyze` + checklist → `speckit-converge` → full validation → commit only after checkpoint passes.

The next feature must not begin while the current feature has unresolved tasks, failing tests, or spec deviations.

## Architectural principles

- `studio-app` is native runtime/host authority; `studio` is toolchain.
- `studio dev` reuses `studio-host`/`studio-shell`/`studio-renderer`, does not launch `studio-app` binary.
- `studio-package` is authoritative bundle format.
- Navigation, guards, lifecycle remain host-controlled; generated artifacts are deterministic, one-owner, drift-checked.
- Reload preserves last known-good surface on failure.

## Immediate next action

The implementation phases `001` through `004` are complete. The next work is release closure: the flagship seams are closed except ticket 34 (generic POS path certification, scoped to the physical baseline run), so what remains are the manual accessibility, hardware/peripheral, live-service, security, provenance, legal, and release-owner gates in `docs/RELEASE_CHECKLIST.md`. The next Spec Kit cycle is 006 (file-based routing), followed by 007, 005, then 008.
