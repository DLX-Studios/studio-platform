# Feature Specification: Unified Native Component Platform

**Feature Branch**: `002-component-platform`

**Created**: 2026-08-05

**Status**: Draft

**Input**: User description: "Add the complete usable component surface from gpui-component, plus selected Flutter-, shadcn/ui-, and adabraka-inspired components that are missing, through the Studio runtime and SDK."

## User Scenarios & Testing

### User Story 1 - Compose Rich Native Layouts (Priority: P1)

Plugin authors can build service catalogs, order summaries, empty states, and dashboards using
consistent layout, typography, display, and status components.

**Why this priority**: Every Studio vertical depends on clear, composable native presentation.

**Independent Test**: Mount a catalog containing cards, badges, avatars, separators, progress,
loading, empty, icon, text, list, and responsive layout nodes and verify the native result and
targeted property updates.

**Acceptance Scenarios**:

1. **Given** a valid display tree, **When** it is mounted, **Then** every supported node renders
   with its declared properties and stable identity.
2. **Given** a display property update, **When** a plugin emits a patch, **Then** only the targeted
   native component changes.
3. **Given** a narrow or expanded host window, **When** layout constraints change, **Then** content
   remains readable without violating tree or size limits.

### User Story 2 - Build Safe Forms and Interactions (Priority: P1)

Plugin authors can collect filters, selections, quantities, options, and non-secret form values
using typed controls with keyboard, pointer, touch, and accessibility behavior.

**Why this priority**: POS, booking, inventory, and retail workflows require reliable input.

**Independent Test**: Exercise the control gallery with keyboard and pointer input, verify typed
events, focus order, validation states, and owner-scoped updates.

**Acceptance Scenarios**:

1. **Given** a supported input control, **When** the operator changes its value, **Then** the guest
   receives exactly one typed event owned by the correct plugin instance.
2. **Given** an invalid property or event, **When** it is submitted, **Then** the host rejects it
   without partially mutating native state.
3. **Given** a secret or PIN field, **When** the operator enters data, **Then** raw secret bytes
   remain host-owned and only readiness/reference state is exposed.

### User Story 3 - Present Trusted Feedback and Overlays (Priority: P1)

Plugin authors can show dialogs, alerts, sheets, popovers, tooltips, notifications, toasts, menus,
and progress states while Studio keeps trusted payment and failure surfaces distinguishable.

**Why this priority**: Checkout recovery and protected actions require clear, accessible feedback.

**Independent Test**: Open, update, dismiss, and recover each overlay from the POS flow while
   checking focus trapping, escape behavior, reduced motion, ownership, and lifecycle cleanup.

**Acceptance Scenarios**:

1. **Given** an overlay request, **When** it is opened, **Then** focus moves predictably and the
   overlay is visually distinct from ordinary plugin content.
2. **Given** a pending protected action, **When** navigation or dismissal is attempted, **Then** the
   host applies the existing confirmation/guard policy.
3. **Given** reduced motion, **When** an overlay transitions, **Then** the same final state appears
   without movement or flashing.

### User Story 4 - Navigate Business Workflows (Priority: P2)

Plugin authors can compose scaffold-style application regions, tabs, sidebars, drawers, breadcrumbs,
steppers, pagination, and appointment/date selection for larger vertical workflows.

**Why this priority**: POS is the first vertical, but the platform must support booking, inventory,
and service-management screens without new host primitives each time.

**Independent Test**: Mount a multi-screen workflow with tabs, sidebar/drawer navigation, breadcrumb
   state, and a date/step selection flow; verify route ownership and state restoration.

**Acceptance Scenarios**:

1. **Given** multiple application regions, **When** the operator navigates between them, **Then** the
   selected region and route-local state remain consistent.
2. **Given** a date, step, or page selection, **When** the value changes, **Then** the plugin receives
   a bounded typed event and the visible selection updates.

### User Story 5 - Display and Operate Business Data (Priority: P2)

Plugin authors can present list tiles, searchable lists, virtual lists, data tables, trees,
description lists, and empty/loading states for catalogs, inventory, staff, and customer history.

**Why this priority**: Vertical applications need more than static cards as their data grows.

**Independent Test**: Render bounded and virtualized collections, sort/filter/select rows, and verify
   stable identity, scrolling, keyboard navigation, and patch performance.

**Acceptance Scenarios**:

1. **Given** a bounded collection, **When** rows are selected or updated, **Then** unaffected rows
   retain identity and interaction state.
2. **Given** a large collection, **When** the operator scrolls or filters, **Then** visible results
   remain responsive and no unbounded guest work is created.

### User Story 6 - Use a Stable Cross-Library SDK Surface (Priority: P1)

Plugin authors can use one Flutter-inspired AssemblyScript catalog even when the native
implementation comes from gpui-component, GPUI primitives, or a Studio-owned composition.

**Why this priority**: The guest contract must remain stable while native implementation libraries
continue to evolve.

**Independent Test**: Generate and compile SDK builders for every catalog kind, compare them with the
   authoritative schemas, and run cross-language golden fixtures.

**Acceptance Scenarios**:

1. **Given** a supported builder, **When** it is compiled and mounted, **Then** its JSON shape matches
   the closed protocol contract.
2. **Given** a library update, **When** native mappings change, **Then** guest builders and protocol
   compatibility remain stable or fail with an explicit version error.

### Edge Cases

- Unknown component kinds, properties, variants, events, or child shapes are rejected before native
  mutation.
- Duplicate IDs, excessive depth, excessive children, oversized text, and patch floods remain
  bounded by the existing protocol limits.
- An overlay whose owner terminates is removed without restoring guest state.
- Focused text inputs, scroll positions, selected rows, and open overlays survive unrelated property
  patches but not a terminal plugin restart.
- A component unavailable on the current host receives a stable capability/error result rather than
  silently rendering a different semantic control.
- All components honor reduced-motion policy and minimum interaction target requirements.

## Requirements

### Functional Requirements

- **FR-001**: Studio MUST expose a closed catalog covering the supported layout, display, form,
  feedback, overlay, navigation, and data components listed in the component plan.
- **FR-002**: Every component MUST declare valid properties, child cardinality, events, focus
  semantics, accessibility labels/states, and reduced-motion behavior.
- **FR-003**: Studio MUST use native gpui-component controls or GPUI primitives when an equivalent
  implementation exists.
- **FR-004**: Studio MUST provide Studio-owned compositions for selected Flutter/shadcn patterns
  missing from gpui-component without importing a second runtime UI library.
- **FR-005**: The guest SDK MUST expose typed builders and helpers for every supported catalog kind.
- **FR-006**: Protocol schemas and SDK bindings MUST be generated from one authoritative contract.
- **FR-007**: Unknown or malformed component messages MUST fail closed before native state changes.
- **FR-008**: Component updates MUST preserve unaffected native identity, focus, scroll, input, and
  selection state.
- **FR-009**: Events MUST be typed, bounded, instance-owned, and delivered at most once per accepted
  user action.
- **FR-010**: Secret input MUST remain host-owned and MUST NOT expose raw values through component
  properties, events, patches, diagnostics, or snapshots.
- **FR-011**: Overlays MUST enforce ownership, focus dismissal, lifecycle cleanup, and pending-action
  navigation guards.
- **FR-012**: Components MUST support keyboard operation, accessible labels/states, pointer input,
  touch-sized targets, and reduced motion where interaction applies.
- **FR-013**: Component additions MUST remain compatible with the Wayland-only, no-X11 runtime
  feature and linkage policy.
- **FR-014**: Component library updates MUST record upstream revisions, Studio deltas, licenses, and
  generated artifact changes.
- **FR-015**: Component tests MUST cover protocol validation, native mapping, targeted updates, SDK
  generation, accessibility, and representative POS usage.
- **FR-016**: The POS example MUST demonstrate the first selected component batches without exposing
  production payment, printer, network, or filesystem capabilities.
- **FR-017**: The catalog MUST remain bounded by existing tree, message, memory, focus, overlay, and
  pending-action limits.

### Key Entities

- **Component Definition**: A closed node kind with properties, children, events, accessibility
  semantics, and native implementation mapping.
- **Component Instance**: An owner-scoped mounted node with stable identity and retained interaction
  state.
- **Component Event**: A typed host-to-guest interaction result bound to one instance and node.
- **Overlay Instance**: An owner-scoped dialog, sheet, popover, tooltip, toast, or menu with focus and
  dismissal policy.
- **Component Catalog Contract**: The generated schema and SDK representation shared by host and
  guest.

## Success Criteria

### Measurable Outcomes

- **SC-001**: Every catalog kind listed as supported has one protocol validation test, one native
  mapping test, and one SDK contract test.
- **SC-002**: The POS example can render the first display/feedback batch without manual HTML/CSS or
  guest DOM access.
- **SC-003**: Keyboard users can operate every interactive component and complete the POS checkout
  flow without pointer input.
- **SC-004**: Invalid component trees and patches produce zero partial native mutations in all tested
  failure cases.
- **SC-005**: A component property update changes only its target and completes within the existing
  interaction budget on the development validation host.
- **SC-006**: Reduced-motion mode produces identical final component states without transition
  movement or flashing.
- **SC-007**: No supported component introduces X11/XCB linkage, guest network/filesystem authority,
  or raw secret exposure.
- **SC-008**: Component additions do not increase the number of unresolved generated schema or SDK
  artifacts after regeneration.
- **SC-009**: A plugin author can build a catalog using the documented SDK surface without depending
  on host internals or native library types.

## Assumptions

- The existing protocol versioning, Wasmtime sandbox, retained UI registry, navigation ownership,
  opaque handles, and no-X11 Wayland host remain the platform foundations.
- gpui-component remains the preferred native source; adabraka-ui is a design/reference source and
  is not added as a runtime dependency.
- Flutter and shadcn names describe guest-facing composition semantics, not a requirement to embed
  Flutter, HTML, CSS, or a browser.
- Hardware-specific benchmark certification remains deferred; automated development-host checks are
  sufficient for this feature’s implementation validation.
- Real payment providers, printers, generic networking, persistent storage, mobile hosts, charts,
  code editors, and docking systems remain outside this feature unless separately specified.
