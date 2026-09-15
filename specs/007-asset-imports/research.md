# Research: Asset Imports and Lucide Icons

**Date**: 2026-09-03.

## R1. Reference shapes (observed in examples)

**Decision**: The typed graph recognizes `iconNode("id", "name")` and `Icon("id", "name")`
call expressions with two string-literal arguments, parsed with `oxc_parser` (0.146 line,
matching the locked transitive version). Anything else shaped — renamed bindings,
comments, member calls (`ui.iconNode(...)`), non-literal arguments — is not a reference.

**Rationale**: Matches exactly what authors write today (`examples/pos-desktop/assembly/
ui.ts`), while comments and renames can no longer create phantom assets.

## R2. No manifest mutation

**Decision**: `collect_lucide`'s manifest rewrite goes away. The build computes
`effective_assets = sorted(manifest.assets ∪ collected icons)` and passes that map to
`pack_bundle`; `manifest.json` is read-only. Collected icons stage under
`build/staging/icons/` (content-addressed by name) instead of being copied into
`assets/icons/`; committed `assets/icons/<name>.svg` files override staged ones silently.

**Rationale**: Builds must never mutate sources; the override convention (`!dst.exists`
skip) is preserved in behavior with a cleaner mechanism.

## R3. Missing icons and budget

**Decision**: A referenced name with no `<name>.svg` in the pinned package fails the
compile phase with `BUILD_ASSET_ICON_MISSING` carrying every referencing span. Total
collected bytes over 512 KiB fail with `BUILD_ASSET_BUDGET_EXCEEDED` naming the overage;
the ceiling and the test assert the same constant.

**Rationale**: Span-accurate failures replace today's silent skip; the budget keeps icon
payloads bounded and is trivially adjustable in one place.

## R4. Package and license

**Decision**: `lucide-static@1.40.0` (ISC) joins `package.json`/`bun.lock` as a build-time
dependency resolved from `node_modules/lucide-static/icons`. Version and license recorded
in `THIRD_PARTY_NOTICES.md`.
