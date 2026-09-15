# Research: File-Based Routing

**Date**: 2026-09-03.

## R1. File conventions (from existing projects)

**Decision**: `routes/index.ts` → `/`; nesting joins segments; `[name]` → `:name` param;
`[...name]` → trailing wildcard; leading-`_` files excluded; `route`/`title` read from
`export const <name> = "..."` string literals only. Observed in
`examples/pos-desktop/routes/pos.ts` (`export const route = "/pos"`, `mount()` entry) and
`examples/github-viewer/routes/github-viewer.ts`.

**Rationale**: File path is the single source of truth; the `route` export is
cross-checked (mismatch diagnostic, path wins) rather than trusted blindly.

## R2. Matching semantics (shared by all three implementations)

**Decision**: Split pattern and path on `/` (normalize trailing slash except root, no case
folding, strip query/fragment before matching). Segments pair left to right: static must
equal; `:param` captures one segment; trailing `*` captures the rest. First match in
registry order wins; registry order is static entries before params before wildcard at each
level (sorted deterministically at build). Miss → not-found entry or explicit miss.

**Rationale**: One rule stated once, implemented three times (Rust tooling, AS SDK,
reload policy), cross-checked by the same 40-path battery.

## R3. Metadata without executing modules

**Decision**: Titles come from `export const title = "..."` string literals (same tiny
scanner as `route`); default is the humanized last segment (`orders` → `Orders`, `/` →
`Home`). No module execution, no TS parsing beyond string-literal export scanning.

## R4. Generated module shape

**Decision**: Keep `route_<name>` consts and `declaredRoutes` byte-identical; append
`RouteEntry[] routeTable` (sorted by path) and `notFoundRoute: string | null`. Content-stable
write (existing 003 pattern).
