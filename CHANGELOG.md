# Changelog

## Unreleased — Project Library isolation

- Add project-scoped Library access through authenticated Designer persistence. Keep catalogs
  and blobs separate per project, reject expired sessions, and serialize concurrent catalog updates.
- Native Library import and binding UI remain in progress.

## Unreleased — Responsive native preview

- Size the canvas to the selected device profile, including safe-area deductions, and scroll
  larger previews inside the editor. Use the same profile to resolve responsive properties.
- Verify profile switching and shared spacing-token edits through measured native controls.

## Unreleased — Native Designer preview

- Share the Runtime renderer with Focus/Workbench preview, including authored text, native inputs,
  selection outlines and prototype events isolated from saved revisions. Fix the toolbar layout
  that squeezed the canvas and bound recursive debug renderer stack usage.
- Implement TimePicker with independent native state, keyboard editing, canonical 24-hour values,
  host updates, disabled editing and a validated native inspector. The catalog has no fallback kinds.
- Add GPUI UI integration tests. Their test-support dependency constrains the shared lockfile to
  Taffy 0.15.0 and unicode-properties 0.1.3; GPUI and GPUI Kit stay at the existing aligned versions.
- Avoid rebuilding the vendored compiler on every Cargo command when its optional upstream
  submodule is absent; document the build-script delta in vendor provenance.
- Confirm the CI repair with successful Linux validation and macOS/Windows transport jobs.

## Unreleased — Designer ownership and live editing

- Bound native project source, workspace, recovery, conflicts and settings to an active
  authenticated owner; project IDs now survive restarts without identity collisions.
- Clear identity-owned editor state and reject obsolete asynchronous results on session changes.
  Legacy records without ownership proof remain intact and require explicit recovery.
- Apply Agent and MCP batches with last-writer-wins semantics, retain explicit preconditions,
  and give every accepted batch its own undo entry. Human undo grouping remains available.
- Updated flagship evidence to schema version 2: `independent_undo`, `batch_name`, and
  `batches_undone` replace the old grouped-agent fields. The demo verifies two separate undos.
- Recorded the immutable Design build contract; native signed Build/Run is still pending.
- Reduced routine CI artifact growth with consistent verification profiles and a shared debug
  startup probe; aligned the headless test with Wayland/X11 display diagnostics.

## Unreleased — Windows reload transport

- Added shared local reload stream/listener types: Unix standard-library sockets and a
  Windows-only `uds_windows` AF_UNIX adapter. Reload wire messages remain at version 1.
- Reused short hashed temporary endpoints on Windows to avoid checkout path limits.
- Updated Runtime, CLI, and reload tests to use the platform adapter; gated Unix stack-limit
  and shell-fixture code, and used the native executable suffix for runtime discovery.
- Added native Windows/macOS reload transport CI tests.

## Unreleased — native platform dependency alignment

- Replaced the five-crate GPUI Kit 0.7.0 Studio fork with published GPUI Kit 0.8.1
  and its matching GPUI 0.3.10 family.
- Enabled upstream native platform backends, including X11 alongside Wayland on Linux.
- Shared the Runtime/Designer display preflight; Linux accepts either backend, while other
  native platforms do not require Wayland environment variables.
- Replaced no-X11 CI gates with platform feature checks and updated constitution 2.0.0
  and ADR-0004. Target-platform release validation remains required.

## Unreleased — gpui-kit 0.7.0 migration

- Moved the vendored component library from the `gpui-component` crates to the `gpui-kit`
  facade at 0.7.0.
- Raised the pinned `gpui-pre` and `gpui-pre-platform` versions from 0.3.5 to 0.3.7.
- Moved component imports to `gpui_kit::component`, assets to `gpui_kit::assets`, and platform
  utilities to `gpui_kit::platform`.
- Replaced `gpui_component::init` with `gpui_kit::init`.
- Removed the `Root::bordered(false)` call sites, which no longer exist in 0.7.0.

## Unreleased — Studio Runtime platform foundation

Studio Platform is the native application runtime monorepo. It owns the protocol, retained UI
model, native GPUI host, plugin execution boundary, component runtime, security model, and
AssemblyScript SDK. It is independent from both the Studio Site product workspace and the Orbit
infrastructure platform.

### Monorepo organization

- Established the native platform layout around `apps/`, `crates/`, `sdk/`, `protocol/`,
  `examples/`, `tests/`, `vendor/`, `docs/`, `scripts/`, and `specs/`.
- Grouped core protocol, UI registry, component runtime, event runtime, and navigation concerns
  under the Studio core boundary.
- Grouped Wasmtime, capability bridging, package loading, security, hardware, and native
  rendering under the host boundary.
- Added component adapter, state registry, overlays, navigation, data, and accessibility areas.
- Added repository-level architecture, contributor, and agent guidance.

### Protocol and native runtime

- Kept the closed protocol catalog as the authoritative contract source.
- Preserved stable node identity, mount trees, patch batches, targeted updates, property
  validation, child-cardinality checks, versioning, and atomic patch application.
- Preserved native GPUI rendering with Wayland-only platform constraints and no X11/XCB fallback.
- Preserved host-owned focus, accessibility, reduced-motion, lifecycle, and event dispatch
  behavior.

### Security and plugin execution

- Preserved Wasmtime execution with no WASI by default.
- Preserved capability-scoped host imports, opaque secret handles, host-owned payment/PIN/password
  values, package signatures, integrity validation, fuel/epoch/memory limits, secure teardown,
  and redaction tests.
- Kept no-X11 linkage checks and headless Wayland validation in the platform workflow.

### Components and SDK

- Established `gpui-component` as the native implementation base through adapters rather than
  duplicating component implementations.
- Added the host-side state-registry direction for stateful controls, tables, trees, editors,
  docking, resizable panels, popovers, selection controls, and virtualized lists.
- Organized the AssemblyScript SDK around public components, fluent builders, typed properties,
  events, signals, derived values, effects, cleanup, generated protocol bindings, opaque handles,
  and test helpers.
- Unified plugin events around a versioned envelope and host-owned lifecycle cleanup.

### Migration and provenance

- Documented upstream inventory, dependency ownership, generated artifacts, licenses, compatibility
  shims, vertical migration slices, and validation requirements.
- Kept navigation, theme, animation, accessibility, sandbox, and routing references as audited
  patterns rather than competing runtime foundations.
- Preserved third-party provenance and notices in `THIRD_PARTY_NOTICES.md`.

### Follow-up

- Complete the remaining vertical slices for adapters, stateful components, navigation, SDK
  surfaces, gallery examples, and POS integration.
- Run the full Rust, Bun, protocol-generation, no-X11, and headless Wayland validation matrix
  before declaring the platform migration complete.
