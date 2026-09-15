# Feature Specification: File-Based Routing

**Feature Branch**: `006-file-routing`

**Created**: 2026-09-03

**Status**: Draft

**Input**: User description: "Define `routes/` static/nested/param/not-found conventions, route metadata, deterministic registry (not just constants), reject duplicates/ambiguous/malformed routes, connect to resolver/SDK, dev/prod coverage. The current scanner only emits `declaredRoutes`."

## User Scenarios & Testing

### User Story 1 - Declare Routes as Files (Priority: P1)

A plugin author creates `routes/pos.ts`, `routes/orders/active.ts`, `routes/orders/[id].ts`,
and `routes/[...rest].ts`, and the toolchain builds a deterministic route registry mapping
each file to its path pattern with parameter names, titles, and source files. Static, nested,
parameterized, and not-found routes all resolve; underscore-prefixed files are helper modules,
never routes.

**Why this priority**: File-based routing is the navigation contract authors program against;
everything else (resolver, SDK, reload policy) consumes the registry.

**Independent Test**: Scan a fixture project with all route shapes plus error cases and verify
the registry entries, ordering, and every rejection.

**Acceptance Scenarios**:

1. **Given** route files for static, nested, param, and catch-all shapes, **When** the registry
   builds, **Then** each maps to its correct pattern with parameter names, titles, and source
   files in deterministic order.
2. **Given** a file starting with underscore, **When** the registry builds, **Then** it is
   excluded without a diagnostic.
3. **Given** two files mapping to the same path, **When** the registry builds, **Then** both
   are rejected naming the duplicate path.
4. **Given** two different parameter names for the same shape, **When** the registry builds,
   **Then** both are rejected as ambiguous naming the shape.
5. **Given** a malformed route file (empty segments, bad characters, unclosed brackets,
   misplaced wildcard), **When** the registry builds, **Then** it is rejected naming the
   problem.

---

### User Story 2 - Resolve Paths Through the Registry (Priority: P1)

Given a concrete path, the resolver returns the matching registry entry with extracted
parameters, or the not-found entry when nothing matches. The same matching semantics run in
the guest SDK, the CLI tooling, and the reload route policy, so a path means the same thing
everywhere.

**Why this priority**: One matching rule across guest, tooling, and host prevents the class
of bugs where navigation succeeds but guards, reloads, or breadcrumbs disagree.

**Independent Test**: Match a battery of paths (static, nested, params, wildcard, misses)
against a fixture registry in all three implementations and compare outcomes.

**Acceptance Scenarios**:

1. **Given** a concrete path, **When** it matches a static or nested entry, **Then** the entry
   returns with no parameters.
2. **Given** a path matching a parameterized entry, **When** it resolves, **Then** the entry
   returns with extracted parameter values.
3. **Given** a path matching nothing, **When** it resolves, **Then** the not-found entry (or
   an explicit miss when none is declared) returns.

---

### User Story 3 - Metadata Travels With Routes (Priority: P2)

Route titles (and future metadata) come from `export const title` / `export const route`
declarations in the route file, defaulting sensibly when absent. Generated surfaces
(breadcrumbs, tabs, the route table) read titles from the registry instead of hardcoding
them.

**Why this priority**: Titles make navigation surfaces data-driven, but authors can ship
without them.

**Independent Test**: Build the registry from files with, without, and with mismatched
declarations; verify titles, defaults, and mismatch diagnostics.

**Acceptance Scenarios**:

1. **Given** a route file declaring a title, **When** the registry builds, **Then** the entry
   carries that title.
2. **Given** a route file without declarations, **When** the registry builds, **Then** the
   entry carries the default title with no diagnostic.
3. **Given** a declared `route` export disagreeing with the file path, **When** the registry
   builds, **Then** a diagnostic names the mismatch and the file path wins.

### Edge Cases

- `routes/index.ts` maps to `/`; trailing `/index` segments collapse; empty route files are
  rejected, not silently skipped.
- A catch-all that is not last in its directory, or two catch-alls, is malformed.
- Parameter names must be valid identifiers; `[...]` with an empty name is malformed.
- Case sensitivity: paths match exactly as declared (no case folding).
- Query strings and fragments never participate in matching; trailing slashes are normalized
  except for the root.
- Registry generation is byte-identical for identical inputs (sorted entries, stable render).

## Requirements

### Functional Requirements

- **FR-001**: The toolchain MUST map `routes/` files to path patterns: `index` → `/`,
  nesting → joined segments, `[name]` → `:name` parameters, `[...name]` → wildcard, leading
  `_` files excluded.
- **FR-002**: The registry MUST carry per-entry pattern, parameter names, title, and source
  file, rendered deterministically (sorted, stable format).
- **FR-003**: Duplicate paths, ambiguous parameter shapes, and malformed routes MUST fail
  with stable diagnostics naming the files and the problem.
- **FR-004**: The resolver MUST match static, nested, parameterized, and wildcard entries,
  extract parameters, and fall back to the not-found entry (or explicit miss).
- **FR-005**: The same matching semantics MUST run in the guest SDK, the CLI tooling, and
  the reload route policy.
- **FR-006**: Titles MUST come from `title` exports when present and default otherwise; a
  disagreeing `route` export MUST produce a mismatch diagnostic with the file path winning.
- **FR-007**: The generated module MUST keep the existing `route_<name>` constants and
  `declaredRoutes` exports unchanged for compatibility.
- **FR-008**: The reload route policy MUST match preserved routes by pattern, not by string
  equality.
- **FR-009**: Every behavioral change MUST be covered by red-green tests per the constitution.

### Key Entities

- **RouteEntry**: Path pattern, segments, parameter names, title, source file, kind
  (static/param/wildcard).
- **RouteRegistry**: Sorted entries plus optional not-found entry; deterministic render.
- **RouteMatch**: Entry plus extracted parameters, or an explicit miss.
- **RouteDiagnostic**: Stable code, message, implicated files.

## Success Criteria

- **SC-001**: A fixture project exercising every route shape builds a registry matching the
  golden file byte-for-byte.
- **SC-002**: Every error family (duplicate, ambiguous, malformed ×4, mismatch) produces its
  documented diagnostic in all tested cases.
- **SC-003**: A 40-path battery resolves identically across the Rust tooling, the guest SDK,
  and the reload policy.
- **SC-004**: Existing `route_<name>`/`declaredRoutes` consumers pass unchanged.
- **SC-005**: A reload to a parameterized current route preserves it when the pattern still
  matches.

## Assumptions

- Route files are TypeScript modules exporting `route` and `mount()` by convention; the
  registry reads only `route`/`title` string exports and file paths, never executes modules.
- The host navigation stack (001/002) remains the runtime navigation authority; the registry
  is build-time data plus a pure matcher.
- Trailing-slash normalization and case-sensitive matching are documented policy, not bugs.
