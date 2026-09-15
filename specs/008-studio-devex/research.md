# Research: Studio Script Developer Experience

**Date**: 2026-09-03.

## R1. Crate composition

**Decision**: Vendor `rsvelte_fmt`, `rsvelte_formatter`, `tailwind_class_order`,
`rsvelte_lint`, `rsvelte_diagnostics`, `rsvelte_preprocess` (manifest-only),
`rsvelte_projection` at pinned rev `b64aed9c` with the same manifest-only lint deltas
as 004. Depend with `default-features = false`, no optional features. Only
`studio-language-server/src/{format,lint,projection}.rs` name these crates.

**Rationale**: Same precedent as 004 (SSH submodule makes git deps unreproducible);
`preprocess` stays manifest-only like `projection` did (optional feature off).

## R2. Format path

**Decision**: `FormatSession::format(source, filepath)` with `.studio` mapped onto the
Svelte pipeline inside our adapter (the session dispatches by extension; Studio owns
the mapping, not upstream). Idempotence asserted by formatting twice in tests.

## R3. Lint layering

**Decision**: `run_native_rules` (or the JSON API for structured output) supplies
Svelte-language diagnostics; a Studio rules module adds catalog/prop/event/subset/
security/route/asset checks reusing the 004 validator where semantics overlap. One
ordered stream, upstream codes preserved verbatim, Studio codes in `STUDIO3xx`.

## R4. Projection use

**Decision**: `ProjectionMap` source↔structure offsets back rename, references, and
definition. Gaps degrade to reported ranges, never silent skips.

## R5. Client shape

**Decision**: `editors/vscode/` with `package.json` (language id, grammar, five
commands), `src/extension.ts` (~150 lines: spawn, handshake, command delegation,
degraded mode), and a TextMate grammar covering script/template/interpolation/control
flow. No bundler, no dependencies beyond `vscode` types (dev-only).
