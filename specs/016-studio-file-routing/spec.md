# Feature Specification: Studio File Routing

**Created**: 2026-11-03
**Status**: Implemented and verified
**Input**: Implement tickets and SvelteKit-like file routing using Svelte-style .studio sources, reusing the existing Rust navigation foundation.

## User Scenarios & Testing

### US1 — Pages from files (P1)
Authors create root, nested, parameterized and catch-all pages and get deterministic routing in development and packaged applications.
Acceptance: +page.studio defines a page; +layout.studio wraps descendant pages without becoming a route. Legacy index.studio and named .studio pages work. Existing .ts routes keep their current meanings. Duplicates and ambiguous paths fail with source-linked errors.

### US2 — Compose screens (P1)
Authors import reusable .studio components, pass typed props, keep independent state per instance and put page content into a shared layout outlet.
Acceptance: nested imports and nested layouts render in order; changing imported source hot reloads; missing imports, cycles, missing required props, unknown props and project-root escapes fail without replacing the last valid preview.

### US3 — Navigate through the host (P1)
Authors request push, replace, pop, pop-to and reset from .studio handlers. The Rust host owns the stack, resolves declared pages, preserves route-local state and handles not-found screens.
Acceptance: root and parameterized routes navigate; back restores state; invalid targets/stack operations fail atomically; dev and packaged Wasm agree. No browser runtime is introduced.

### US4 — Runnable stream example (P2)
The new stream-countdown example demonstrates root and live routes, a shared layout and imported components. The countdown runs automatically from 600 seconds; completion fades the clock and then navigates to the live route. Native timer evidence is separate from source validation.

## Requirements

- **FR-001**: Discover .studio pages using root/nested/index/parameter/catch-all conventions and exclude layouts and private helpers.
- **FR-002**: Reject duplicate, ambiguous, malformed and escaping declarations; deterministic metadata names actual source files.
- **FR-003**: Compose ancestor layouts and imported components with an explicit child outlet, scoped identities, props and reactive state.
- **FR-004**: Reuse studio-navigation for concrete path resolution and bounded host-owned history, including root and catch-all paths.
- **FR-005**: Deliver identical navigation behavior through development IR and packaged Wasm without browser globals or arbitrary guest-native access.
- **FR-006**: Preserve compatible route-local state and last-good behavior across source edits; watch page/layout/component additions and removals.
- **FR-007**: Keep existing single-file .studio and .ts examples working; do not edit generated output as source.
- **FR-008**: Publish tickets, authoring instructions, evidence and an example; report remaining acceptance work accurately.

- **FR-009**: A source-only Timer declaration MUST drive bounded host-scheduled ticks, stop on route exit, catch up elapsed intervals with at most 32 events per poll, and deliver completion once after an optional exit-animation delay. The stream example MUST count down automatically from 600 seconds and navigate to its live route.

- **FR-010**: The stream landing page MUST fill the viewport with its local sunset background and use oversized readable header/clock text. Presentation controls remain closed native props: Box.background_asset, Text.font_size 8–256, Text.color six-digit hex. Clock opacity transitions must interpolate in native rendering, including reduced-motion resolution.

## Success Criteria

- **SC-001**: Automated root/nested/parameter/catch-all and negative-path fixtures pass through the shared router.
- **SC-002**: Imported instances have independent state and changing a dependency invalidates the composed page.
- **SC-003**: A two-page journey navigates forward/back with state restoration through both compiler paths.
- **SC-004**: Existing routing and compiler regression suites pass alongside the new feature.

## Edge Cases and Assumptions

File paths are authoritative. Layouts use a default child outlet; no server load functions, browser SSR, npm execution, or HTTP endpoints are implied. Imports are static, local .studio default component imports. Closed existing navigation commands remain the wire contract; generated, signed project routing metadata is bounded and validated. Imported module count, source bytes and recursion are bounded. This feature does not close unrelated Designer UI/build/signing acceptance tickets 44/68/69.
