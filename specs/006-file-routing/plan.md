# Implementation Plan: File-Based Routing

**Branch**: `006-file-routing` | **Date**: 2026-09-03 | **Spec**: [spec.md](spec.md)

## Summary

Replace the constants-only route scanner with a real registry: `studio-cli/src/routes.rs`
parses `routes/` files into `RouteEntry` records (pattern, params, title, file, kind),
validates duplicates/ambiguity/malformed shapes with stable diagnostics, and renders the
deterministic module (keeping `route_<name>` + `declaredRoutes` for compatibility, adding the
sorted `routeTable` + `notFoundRoute`). The guest SDK gains a pure `matchRoute` over the same
semantics; the reload policy matches preserved routes by pattern.

## Technical Context

**Language/Version**: Rust nightly-2026-03-04; AssemblyScript 0.28.20 for the SDK addition

**Primary Dependencies**: Existing `studio-cli` build pipeline, `studio-protocol` (no new
crates), SDK `navigation.ts`

**Storage**: N/A; registry is a generated build artifact

**Testing**: cargo integration tests (scanner matrix, golden render, matcher battery),
Bun SDK test for `matchRoute`, reload policy test

**Target Platform**: Linux desktop/POS; toolchain host-side

**Project Type**: CLI toolchain inside the native desktop workspace

**Performance Goals**: Registry builds are file-count linear; matcher is segment-linear

**Constraints**: Generated output stays byte-identical for identical inputs; existing
`declaredRoutes` consumers unaffected; no new capabilities

**Scale/Scope**: Three example projects; route files number in the tens

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

PASS. No host/runtime changes (registry is build data + pure matching). Test-first with
matrix, golden, and battery coverage. Additive-only contracts (new table exports; old
constants untouched). No new network/storage/capability paths.

## Project Structure

```text
specs/006-file-routing/
├── plan.md, research.md, data-model.md, quickstart.md, contracts/, tasks.md

crates/studio-cli/src/routes.rs          # entries, scan, validate, render, match (new)
crates/studio-cli/src/build/mod.rs       # generate_routes uses the registry (extend)
crates/studio-cli/tests/route_registry.rs# scanner matrix + golden render (new)
sdk/assemblyscript/assembly/navigation.ts# RouteTable + matchRoute (extend)
sdk/assemblyscript/tests/navigation-routes.test.ts  # SDK battery (new)
crates/studio-app/src/reload.rs          # pattern matching for preserved routes (extend)
```

**Structure Decision**: Registry logic lives in `studio-cli` next to the existing scanner;
matching semantics are mirrored (not shared — different languages) with a cross-checked
battery. No new crates.

## Complexity Tracking

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|-------------------------------------|
| None | N/A | Fits existing boundaries. |
