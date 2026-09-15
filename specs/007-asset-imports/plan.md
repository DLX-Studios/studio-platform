# Implementation Plan: Asset Imports and Lucide Icons

**Branch**: `007-asset-imports` | **Date**: 2026-09-03 | **Spec**: [spec.md](spec.md)

## Summary

Replace the regex lucide scan with a typed reference graph (`studio-cli/src/assets.rs`)
parsed via `oxc_parser` (already in the lockfile through `rsvelte`; added as a direct
dependency): `iconNode("id", "name")` / `Icon("id", "name")` call shapes resolve to icon
names with source spans. Collection copies only referenced SVGs from the pinned
`lucide-static@1.40.0` (new Bun dependency), honors committed `assets/icons/` overrides,
never rewrites `manifest.json` (effective sorted asset set feeds `pack_bundle` directly),
and enforces a byte ceiling with diagnostics. License recorded in `THIRD_PARTY_NOTICES.md`.

## Technical Context

**Language/Version**: Rust nightly-2026-03-04; Bun 1.3.9; `lucide-static@1.40.0` (ISC)

**Primary Dependencies**: `oxc_parser` + `oxc_span` (direct, crates.io, versions matching the
locked 0.146 line), `studio-package` pack path, existing build pipeline

**Storage**: N/A; collected icons are build artifacts under `build/` staging

**Testing**: cargo integration tests (reference matrix, determinism, overrides, budget),
Bun install for the pinned package

**Target Platform**: Linux desktop/POS; toolchain host-side

**Project Type**: CLI toolchain inside the native desktop workspace

**Performance Goals**: Collection is file-count linear; full build stays under existing budgets

**Constraints**: No manifest mutation; byte-identical bundles; missing icons fail with spans;
ISC license recorded; no new capabilities

**Scale/Scope**: Three example projects; icon sets in the tens of files

## Constitution Check

PASS. Test-first with matrix/determinism/override/budget coverage. No host/runtime changes
(build-time asset staging only). Additive contracts (new diagnostic codes, new manifest
reading path — manifest schema untouched). License recorded for provenance.

## Project Structure

```text
specs/007-asset-imports/
├── plan.md, research.md, data-model.md, quickstart.md, contracts/, tasks.md

crates/studio-cli/src/assets.rs        # reference graph, collector, budget (new)
crates/studio-cli/src/build/mod.rs     # use collector; stop mutating manifest.json (extend)
crates/studio-cli/tests/asset_icons.rs # matrix + determinism + overrides + budget (new)
crates/studio-cli/tests/fixtures/icons/# fixture project (new)
package.json + bun.lock                # lucide-static@1.40.0 (extend)
THIRD_PARTY_NOTICES.md                 # lucide license entry (extend)
```

**Structure Decision**: Asset logic lives in `studio-cli` next to the pipeline it feeds; the
pack step consumes the computed asset set. No new crates.

## Complexity Tracking

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|-------------------------------------|
| New direct dep `oxc_parser` | Typed parsing replaces regex; already in the lockfile via rsvelte | Hand-rolled parsing would reimplement an AST walker badly |
