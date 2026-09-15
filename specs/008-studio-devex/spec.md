# Feature Specification: Studio Script Developer Experience

**Feature Branch**: `008-studio-devex`

**Created**: 2026-09-03

**Status**: Draft

**Input**: User description: "Rust-only editor tooling: compose the rsvelte_fmt, rsvelte_lint, and rsvelte_projection Rust crates into studio-language-server with Studio-owned intelligence (completion, hover, definition, rename, references, diagnostics, routes, assets); ship a thin VS Code client that spawns the Rust server over stdio. No JavaScript toolchain packages."

## User Scenarios & Testing

### User Story 1 - Format, Lint, and Navigate .studio Files (Priority: P1)

An author editing `.studio` files gets deterministic formatting, Svelte-language diagnostics
plus Studio rules (catalog components, typed props/events, portable subset, security
boundaries, routes, assets), go-to-definition across modules, rename and reference search,
all from the local language server with no network and no Node.js involved.

**Why this priority**: This is the daily authoring loop; everything else is packaging.

**Independent Test**: Drive the server over stdio with format, diagnostic, definition,
rename, and reference requests against fixture projects and compare against goldens.

**Acceptance Scenarios**:

1. **Given** an unformatted `.studio` file, **When** formatting is requested, **Then** the
   server returns deterministic output that is stable when reformatted.
2. **Given** a file with Svelte-language and Studio-rule violations, **When** diagnostics
   are requested, **Then** every problem reports with code, message, and exact span in one
   ordered stream.
3. **Given** a symbol used across modules, **When** rename or references are requested,
   **Then** every occurrence updates or lists with correct source locations.
4. **Given** a component, prop, event, rune, route, or asset token, **When** completion or
   hover is requested, **Then** catalog-backed items and documentation return.

---

### User Story 2 - Thin Editor Client (Priority: P2)

A VS Code extension registers `.studio`, launches the Rust server, supplies syntax
highlighting, and exposes format/check/build commands. It contains no compiler logic and
degrades to highlighting plus local commands when language features are unavailable.

**Why this priority**: Distribution to authors, after the server works.

**Independent Test**: Install the packaged extension against fixture projects; verify
activation, highlighting, command execution, and graceful degradation with the server
killed.

**Acceptance Scenarios**:

1. **Given** the installed extension, **When** a `.studio` file opens, **Then** the server
   starts and diagnostics appear.
2. **Given** the server unavailable, **When** a file opens, **Then** highlighting and local
   commands still work.
3. **Given** a format command, **When** it runs, **Then** it delegates to the server (or the
   `studio` binary) and never reformats destructively on failure.

---

### User Story 3 - Tooling Failures Never Touch the Compiler (Priority: P1)

Upstream tooling failures (parse divergence, formatter errors, projection gaps) degrade to
clear diagnostics while the Studio validator, IR, and release builds keep working
unchanged. Compatibility fixtures pin every Svelte shape Studio relies on.

**Why this priority**: The editor must never become a second compiler or a release blocker.

**Independent Test**: Inject upstream failures (malformed inputs, version skew fixtures) and
verify the Studio pipeline answers identically with tooling diagnostics attached.

**Acceptance Scenarios**:

1. **Given** input the upstream formatter rejects, **When** formatting is requested, **Then**
   a diagnostic reports it and the source is returned unchanged.
2. **Given** a projection gap, **When** rename is requested, **Then** the affected range is
   reported instead of silently skipped.
3. **Given** any tooling failure, **When** `studio build` runs on the same sources, **Then**
   the build behaves exactly as without the tooling.

### Edge Cases

- Malformed inputs never crash the server; every request gets a response.
- Concurrent requests are serialized deterministically; slow upstream calls cannot
  reorder diagnostics.
- Unknown file types and empty files return empty results, not errors.
- The server starts with no project context and discovers roots lazily.
- Formatter output for identical inputs is byte-identical across runs and machines.

## Requirements

### Functional Requirements

- **FR-001**: The server MUST format `.studio` sources through the `rsvelte_fmt` Rust
  crate with Studio recognition, deterministically and idempotently.
- **FR-002**: The server MUST combine Svelte-language diagnostics (`rsvelte_lint`) with
  Studio rules (catalog, props/events, subset, security, routes, assets) in one ordered
  stream with stable codes and spans.
- **FR-003**: The server MUST provide definition, rename, and references across `.studio`
  modules, routes, and assets backed by projection mappings.
- **FR-004**: The server MUST provide completion and hover from the closed catalog, SDK,
  runes, routes, and assets.
- **FR-005**: The VS Code client MUST spawn the Rust server over stdio, contain no
  compiler logic, and degrade gracefully without it.
- **FR-006**: Upstream failures MUST degrade to diagnostics; the Studio validator, IR, and
  release builds MUST behave identically with or without tooling.
- **FR-007**: Compatibility fixtures MUST pin `.studio` parsing, formatting stability,
  diagnostics, projection mappings, catalog completion, and source locations.
- **FR-008**: Tooling crates MUST be pinned and hidden behind Studio-owned adapters; no
  JavaScript toolchain package may enter the dependency closure.
- **FR-009**: Every behavioral change MUST be covered by red-green tests per the
  constitution.

### Key Entities

- **FormatRequest/Response**: Source plus filepath in, formatted source or diagnostic out.
- **DiagnosticStream**: Ordered merged diagnostics (upstream + Studio) with codes and spans.
- **ProjectionMapping**: Source↔structure offset maps backing rename/references/definition.
- **CompletionCatalog**: Closed catalog/SDK/rune/route/asset items with documentation.
- **ClientSession**: Extension lifecycle: server spawn, capability handshake, degradation.

## Success Criteria

- **SC-001**: Fixture projects format byte-identically across repeated runs and stay stable
  when reformatted.
- **SC-002**: Every diagnostic family (Svelte + Studio rules) reports with code and span;
  cross-tool fixtures agree on locations.
- **SC-003**: Rename/references/definition resolve correctly across modules, routes, and
  assets in all tested cases.
- **SC-004**: The packaged extension activates, highlights, commands, and degrades per
  acceptance in manual verification.
- **SC-005**: Injected upstream failures change nothing about compiler/IR/build behavior.

## Assumptions

- The `rsvelte_fmt`, `rsvelte_lint`, and `rsvelte_projection` Rust crates vendor from the
  pinned rev on demand, following the 004 precedent (manifest-only deltas recorded).
- `studio-language-server`'s existing Workspace index and stdio transport are the
  foundation; this feature extends them.
- VS Code is the only editor client in scope; other editors reuse the stdio protocol.
- No browser preview, no NAPI, no Vite, no tsc-based checking in this feature.
