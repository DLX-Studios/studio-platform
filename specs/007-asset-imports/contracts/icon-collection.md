# Contract: Icon Collection

**Status**: Contract for feature 007. Additive-only after landing.

## Reference shapes

`iconNode("id", "name")` and `Icon("id", "name")` with two string literals. Renames,
member calls, comments, and non-literals are not references.

## Diagnostics

- `BUILD_ASSET_ICON_MISSING`: referenced name absent from the pinned package; carries
  every referencing span.
- `BUILD_ASSET_BUDGET_EXCEEDED`: collected bytes over 512 KiB; names the overage.
- `BUILD_ASSET_PACKAGE_MISSING`: pinned package not installed; environment failure.

## Staging

Collected icons stage under `<project>/build/staging/icons/<name>.svg`
(content-addressed by name). Committed `assets/icons/<name>.svg` files override staged
ones silently. `manifest.json` is never written. Effective asset set is sorted
manifest-declared entries plus staged icons.
