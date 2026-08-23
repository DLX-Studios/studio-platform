# Changelog

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
