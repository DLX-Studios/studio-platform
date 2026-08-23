# Tasks: Unified Native Component Platform

**Input**: Design documents from `specs/002-component-platform/`

**Tests**: Required by the Studio constitution; every behavioral task follows red-green-refactor.

## Phase 1: Setup

- [X] T001 Add the component-platform feature references and generated-artifact ownership notes in `docs/COMPONENT_CATALOG_PLAN.md`, `specs/002-component-platform/`, and `tests/foundation.test.ts`.
- [X] T002 [P] Add the component catalog validation commands to `docs/development/BUILDING.md` and `specs/002-component-platform/quickstart.md`.

## Phase 2: Foundational

- [X] T003 Add failing protocol catalog inventory tests for all supported kinds, property closures, child rules, and stable error families in `crates/studio-protocol/tests/component_catalog_v1.rs`.
- [X] T004 Implement the authoritative component inventory and shared semantic validation in `crates/studio-protocol/src/ui.rs`, `crates/studio-protocol/src/properties.rs`, and `crates/studio-protocol/src/error.rs`.
- [X] T005 [P] Add failing native-layer coverage for component mapping, focus semantics, accessibility state, reduced motion, and owner-scoped events in `crates/studio-components/tests/component_catalog.rs`.
- [X] T006 [P] Add failing SDK catalog coverage for every component builder, typed property, event, stable ID, and compatibility alias in `sdk/assemblyscript/tests/component-catalog.test.ts`.
- [X] T007 Regenerate schemas, fixtures, and AssemblyScript bindings from the authoritative protocol and make generated-artifact drift tests pass in `protocol/`, `sdk/assemblyscript/assembly/generated/`, and `scripts/generate-protocol.ts`.

## Phase 3: User Story 1 — Rich Native Layouts (P1) 🎯 MVP

**Independent test**: Mount the display gallery and POS catalog with native cards, status, loading,
empty, list, and responsive layout nodes; verify targeted updates and retained identity.

- [X] T008 [P] [US1] Add failing display-gallery integration coverage for Card, Badge, Tag, Separator, Avatar, Empty, Skeleton, Progress, ProgressCircle, Spinner, and layout aliases in `tests/integration/component_display.rs`.
- [X] T009 [US1] Implement native display mappings using GPUI/gpui-component in `crates/studio-components/src/catalog.rs`, `crates/studio-components/src/update.rs`, and `crates/studio-components/src/display.rs`.
- [X] T010 [US1] Add typed AssemblyScript builders and bindings for the display batch in `sdk/assemblyscript/assembly/components/display.ts` and `sdk/assemblyscript/assembly/components/layout.ts`.
- [X] T011 [US1] Replace POS service and order-summary manual panel styling with Card, Badge, Separator, Avatar, and Empty nodes in `examples/pos-desktop/assembly/` and its generated fixtures.
- [X] T012 [US1] Verify display-batch accessibility, reduced motion, retained updates, POS rendering, and generated artifacts with `cargo test --test component_display`, `bun test`, and `bun run build:pos`.

## Phase 4: User Story 2 — Safe Forms and Interactions (P1)

**Independent test**: Operate the control gallery by keyboard and pointer and verify typed,
owner-scoped events and host-owned secret behavior.

- [X] T013 [P] [US2] Add failing protocol and native tests for Checkbox, Radio, Switch, Toggle, ButtonGroup, RangeSlider, TextArea, Field, InputGroup, Combobox, NumberInput, and OtpInput in `crates/studio-protocol/tests/ui_properties_v1.rs` and `crates/studio-components/tests/interactive_catalog.rs`.
- [X] T014 [US2] Implement native control mappings and typed event dispatch in `crates/studio-components/src/controls.rs`, `crates/studio-components/src/events.rs`, and `crates/studio-components/src/catalog.rs`.
- [X] T015 [US2] Add SDK builders and helper types for forms and selection in `sdk/assemblyscript/assembly/components/interaction.ts`, `sdk/assemblyscript/assembly/widgets.ts`, and `sdk/assemblyscript/tests/interactive-catalog.test.ts`.
- [X] T016 [US2] Extend the POS search, category filter, quantity, and payment-input examples with the selected controls in `examples/pos-desktop/assembly/`.
- [X] T017 [US2] Verify keyboard, pointer, touch-sized targets, accessibility states, owner rejection, and secret isolation with focused Rust/Bun suites.

## Phase 5: User Story 3 — Trusted Feedback and Overlays (P1)

**Independent test**: Open and dismiss every overlay from the checkout flow while checking focus,
ownership, reduced motion, and pending-action guards.

- [X] T018 [P] [US3] Add failing overlay contract and lifecycle tests for Tooltip, Popover, Dialog, AlertDialog, Sheet, Toast, Notification, Banner, ContextMenu, and CommandPalette in `crates/studio-protocol/tests/overlay_v1.rs` and `tests/integration/component_overlays.rs`.
- [X] T019 [US3] Implement host-owned overlay mappings, focus traps, dismissal rules, and lifecycle cleanup in `crates/studio-components/src/overlays.rs`, `crates/studio-app/src/confirmation_surface.rs`, and `crates/studio-app/src/failure_surface.rs`.
- [X] T020 [US3] Add SDK overlay builders and event correlations in `sdk/assemblyscript/assembly/components/overlay.ts` and `sdk/assemblyscript/assembly/events.ts`.
- [X] T021 [US3] Integrate trusted payment confirmation, recoverable failure, toast, and receipt-preview states with the overlay batch in `crates/studio-app/src/` and `examples/pos-desktop/assembly/`.
- [X] T022 [US3] Verify focus, escape/dismissal, reduced motion, pending-payment guards, accessibility announcements, and compositor-loss cleanup.

## Phase 6: User Story 4 — Business Navigation (P2)

- [X] T023 [P] [US4] Add failing navigation component tests for Scaffold, AppBar, Sidebar, NavigationBar, NavigationRail, Drawer, Tabs, Breadcrumb, Stepper, and Pagination in `tests/integration/component_navigation.rs`.
- [X] T024 [US4] Implement navigation component mappings and route ownership integration in `crates/studio-components/src/navigation.rs`, `crates/studio-app/src/router.rs`, and `crates/studio-navigation/src/`.
- [X] T025 [US4] Add SDK navigation builders and typed selection events in `sdk/assemblyscript/assembly/navigation.ts` and `sdk/assemblyscript/assembly/components/interaction.ts`.
- [X] T026 [US4] Add a multi-screen appointment/settings demonstration and verify route-local state restoration in `examples/pos-desktop/assembly/` and `tests/integration/checkout_navigation.rs`.

## Phase 7: User Story 5 — Business Data (P2)

- [X] T027 [P] [US5] Add failing list/data tests for ListTile, SearchableList, VirtualList, DataTable, Tree, DescriptionList, Calendar, DatePicker, and TimePicker in `tests/integration/component_data.rs`.
- [X] T028 [US5] Implement bounded native data mappings, sorting/filtering/selection events, virtualization limits, and calendar validation in `crates/studio-components/src/data.rs` and `crates/studio-components/src/catalog.rs`.
- [X] T029 [US5] Add SDK builders and generated fixtures for data and scheduling components in `sdk/assemblyscript/assembly/components/display.ts`, `sdk/assemblyscript/assembly/components/interaction.ts`, and `protocol/fixtures/`.
- [X] T030 [US5] Verify retained row identity, scrolling, keyboard navigation, bounded large collections, and POS/inventory demonstration coverage.

## Phase 8: User Story 6 — Stable Cross-Library SDK (P1)

- [X] T031 [P] [US6] Add cross-language golden fixtures for every component envelope, property variant, event, and compatibility alias in `protocol/fixtures/protocol-v1/` and `sdk/assemblyscript/tests/generated-contract.test.ts`.
- [X] T032 [US6] Implement component version/error reporting and SDK compatibility guards in `crates/studio-protocol/src/`, `crates/studio-app/src/diagnostic.rs`, and `sdk/assemblyscript/assembly/errors.ts`.
- [X] T033 [US6] Verify a clean protocol regeneration, starter/plugin author flow, and documented component quickstart with `bun run generate:protocol`, `bun test`, and `specs/002-component-platform/quickstart.md`.

## Phase 9: Polish and Cross-Cutting Validation

- [X] T034 [P] Update upstream component provenance, license notices, and the component catalog matrix after each synchronized upstream revision in `vendor/gpui-component/UPSTREAM.md`, `docs/upstream/`, and `THIRD_PARTY_NOTICES.md`.
- [X] T035 [P] Add full accessibility and reduced-motion acceptance coverage for the expanded catalog in `tests/e2e/accessibility.rs` and `docs/accessibility/ACCEPTANCE.md`.
- [X] T036 Run `cargo fmt`, `cargo clippy --locked --workspace --all-targets`, `cargo test --locked --workspace`, `bun run check`, `bun test`, no-X11 checks, and the POS quickstart; record results in the feature validation report.

## Dependencies and execution order

Foundational tasks T003–T007 block all stories. US1 is the MVP and enables the display gallery.
US2 and US3 can proceed in parallel after the foundation. US4 and US5 depend on the shared catalog
and event model but are independently testable. US6 and polish follow the component batches.

## MVP scope

T003–T012: the authoritative catalog foundation plus the first native display/feedback batch in the
POS example.
