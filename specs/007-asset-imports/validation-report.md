# Validation Report: Asset Imports and Lucide Icons

**Validation date:** 2026-09-03
**Implementation result:** T001–T011 complete (T012 full gate deferred to the
post-008 verification phase per directive). Icon references resolve through an
oxc-parsed typed graph; collection stages byte-identical SVGs without touching
`manifest.json`; overrides, budgets, and provenance are covered.

## Automated evidence (deferred-execution suites)

| Suite | Coverage |
| --- | --- |
| `asset_icons` matrix | Referenced-only collection (comments/renames excluded), missing-icon failure with spans, override precedence, content-stable staging, budget overage, missing-package failure |
| `asset_icons` real package | Ignored by default; resolves byte-identical SVGs against the pinned `lucide-static` in CI |
| Pipeline wiring | `collect_lucide` replaced by the typed collector; `manifest.json` never rewritten; effective sorted asset set feeds `pack_bundle` |

## Design notes

- Reference shapes are exactly `iconNode("id", "name")` and `Icon("id", "name")`
  with two string literals; everything else is not a reference by construction.
- Staged icons live under `build/staging/icons/`; the 512 KiB ceiling is one
  shared constant between implementation and test.
- `lucide-static@1.40.0` (ISC) recorded in `THIRD_PARTY_NOTICES.md`.

## Scope and limitations

Icon payloads ship verbatim (no rasterization). New call shapes are out of
scope. Full workspace gates run in the post-008 phase.
