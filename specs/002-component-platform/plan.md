# Implementation Plan: Unified Native Component Platform

**Branch**: `002-component-platform` | **Date**: 2026-08-05 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/[###-feature-name]/spec.md`

**Note**: This template is filled in by the `$speckit-plan` command; its definition describes the execution workflow.

## Summary

Add a versioned, closed component catalog spanning the latest audited gpui-component surface and
selected Flutter/shadcn composition patterns. Rust protocol types remain authoritative; native
GPUI/gpui-component mappings, AssemblyScript builders, generated schemas, and POS demonstrations
ship in dependency-ordered vertical slices.

## Technical Context

<!--
  ACTION REQUIRED: Replace the content in this section with the technical details
  for the project. The structure here is presented in advisory capacity to guide
  the iteration process.
-->

**Language/Version**: Rust nightly-2026-03-04, AssemblyScript 0.28.20, Bun 1.3.9+

**Primary Dependencies**: GPUI/Zed pinned revision, vendored gpui-component, Wasmtime, Studio protocol and SDK

**Storage**: N/A; component state is retained in the host instance and guest state

**Testing**: cargo test, Bun tests, generated-schema drift tests, no-X11 checks, accessibility and POS integration tests

**Target Platform**: Linux desktop/POS on native Wayland; no X11/XWayland fallback

**Project Type**: Sandboxed native desktop runtime with AssemblyScript/WASM plugins

**Performance Goals**: Preserve existing interaction budgets, retained identity, idle zero-work, and 60 FPS transition policy on the development validation host

**Constraints**: Closed protocol, bounded trees/messages/queues, host-owned secrets, no guest DOM, no new capabilities, no X11 linkage

**Scale/Scope**: All gpui-component controls that can be safely exposed, plus selected Flutter/shadcn compositions; charts/editor/docking/mobile remain deferred

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

PASS. The plan preserves host authority, test-first contract changes, generated schemas, retained
native rendering, Wayland-only linkage, bounded resources, and small traceable slices. No new
capability, secret path, network path, storage path, or protocol escape is introduced.

## Project Structure

### Documentation (this feature)

```text
specs/[###-feature]/
├── plan.md              # This file ($speckit-plan command output)
├── research.md          # Phase 0 output ($speckit-plan command)
├── data-model.md        # Phase 1 output ($speckit-plan command)
├── quickstart.md        # Phase 1 output ($speckit-plan command)
├── contracts/           # Phase 1 output ($speckit-plan command)
└── tasks.md             # Phase 2 output ($speckit-tasks command - NOT created by $speckit-plan)
```

### Source Code (repository root)
<!--
  ACTION REQUIRED: Replace the placeholder tree below with the concrete layout
  for this feature. Delete unused options and expand the chosen structure with
  real paths (e.g., apps/admin, packages/something). The delivered plan must
  not include Option labels.
-->

```text
crates/studio-protocol/        # authoritative kinds, properties, events, limits
crates/studio-components/      # native mapping, focus, accessibility, retained updates
sdk/assemblyscript/             # guest builders and typed events
protocol/                       # generated JSON schemas and protocol fixtures
examples/pos-desktop/           # vertical demonstrations (pos-checkout removed, pos-desktop is keeper)
tests/                          # integration, e2e, accessibility, and traceability tests
docs/                           # catalog matrix, upstream provenance, and quickstarts
```

**Structure Decision**: Extend the existing protocol → native mapping → SDK → generated-artifact
pipeline. Each component batch crosses these layers together and is demonstrated in the POS
example when it changes visible behavior.

## Complexity Tracking

> **Fill ONLY if Constitution Check has violations that must be justified**

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|-------------------------------------|
| None | N/A | The component platform fits the existing workspace boundaries. |
| [e.g., Repository pattern] | [specific problem] | [why direct DB access insufficient] |
