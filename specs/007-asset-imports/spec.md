# Feature Specification: Asset Imports and Lucide Icons

**Feature Branch**: `007-asset-imports`

**Created**: 2026-09-03

**Status**: Draft

**Input**: User description: "Collect only referenced lucide-static assets via typed import graph (not regex), deterministic bundle without hand-editing manifest.json, user overrides, missing-icon diagnostics, license, bundle-size tests."

## User Scenarios & Testing

### User Story 1 - Referenced Icons Only, Found by Analysis (Priority: P1)

A plugin author references icons by name in assembly sources (`iconNode("brand-icon",
"store")`). The toolchain collects exactly those icons from the pinned `lucide-static`
package using a typed syntax analysis — not pattern matching — so renames, comments, and
string'impostors never create phantom assets or miss real ones. An icon with no source file
fails the build with a diagnostic pointing at the reference.

**Why this priority**: Correctness of what ships; everything else refines it.

**Independent Test**: Fixture project with referenced, commented-out, renamed, and missing
icons; verify the collected set equals exactly the referenced set and the missing one fails
with a span.

**Acceptance Scenarios**:

1. **Given** sources referencing icons by name, **When** the build collects, **Then** exactly
   the referenced icons land in the bundle, byte-identical to the package files.
2. **Given** an icon name in a comment or a renamed binding, **When** the build collects,
   **Then** it is not collected.
3. **Given** a referenced name with no file in the pinned package, **When** the build runs,
   **Then** it fails with a diagnostic naming the icon and its source span.

---

### User Story 2 - Deterministic Bundles, Untouched Manifests, Honored Overrides (Priority: P1)

The author's `manifest.json` is never rewritten by the build: the effective asset set is
computed (declared manifest assets plus collected icons, sorted) and passed to packaging.
Committed files under `assets/icons/` override collected ones byte-for-byte, so custom art
always wins without configuration.

**Why this priority**: Builds must never mutate sources; overrides are the custom-art story.

**Independent Test**: Build twice and compare bundles; commit an override and verify it wins;
verify the manifest file is untouched.

**Acceptance Scenarios**:

1. **Given** identical inputs, **When** two builds run, **Then** the asset bytes in both
   bundles are identical and `manifest.json` is byte-identical to before the build.
2. **Given** a committed `assets/icons/<name>.svg` differing from the package, **When** the
   build collects, **Then** the committed file ships and no diagnostic fires.
3. **Given** manifest-declared assets, **When** the build runs, **Then** all declared assets
   ship alongside collected icons in sorted order.

---

### User Story 3 - Licensed, Bounded Icon Sets (Priority: P2)

The pinned `lucide-static` version and its ISC license are recorded for the provenance
review, and a bundle-size test keeps icon payloads bounded so a stray wildcard reference
cannot bloat bundles silently.

**Why this priority**: Legal traceability and payload hygiene, after correctness.

**Independent Test**: Provenance entry exists with version and license; a budget test fails
when the collected set exceeds the ceiling.

**Acceptance Scenarios**:

1. **Given** the pinned package, **When** the provenance review runs, **Then** version and
   license are recorded.
2. **Given** a collected icon set, **When** its total bytes exceed the documented ceiling,
   **Then** the build fails naming the overage.

### Edge Cases

- Icon names are matched exactly (case-sensitive); `Store` ≠ `store`.
- An empty reference set collects nothing and emits no diagnostics.
- Duplicate references across files collect once.
- A `lucide-static` package that cannot be found fails in the compile phase with an
  environment diagnostic, not a silent skip.
- Non-SVG files in `assets/icons/` overrides ship verbatim; only `*.svg` names participate
  in collection.
- Bundle-size accounting covers collected icons only, not author assets.

## Requirements

### Functional Requirements

- **FR-001**: The toolchain MUST resolve icon references by parsing assembly sources into a
  typed reference graph (call shapes with source spans), never by regex.
- **FR-002**: The toolchain MUST collect exactly the referenced icons present in the pinned
  package, byte-identical.
- **FR-003**: Missing icons MUST fail the build with a diagnostic naming the icon and its
  source span.
- **FR-004**: The build MUST NOT modify `manifest.json`; the effective asset set (declared
  plus collected, sorted) MUST feed packaging directly.
- **FR-005**: Committed `assets/icons/<name>.svg` files MUST override collected ones
  silently.
- **FR-006**: Repeated builds MUST produce byte-identical icon bytes in bundles.
- **FR-007**: The pinned package version and license MUST be recorded for provenance.
- **FR-008**: Collected icon payloads MUST respect a documented byte ceiling, enforced by a
  failing test and a build diagnostic.
- **FR-009**: Every behavioral change MUST be covered by red-green tests per the
  constitution.

### Key Entities

- **IconReference**: Icon name plus every referencing source span.
- **IconCollection**: Resolved name→bytes map with override provenance per entry.
- **AssetSet**: Effective sorted bundle asset list (declared + collected).
- **IconBudget**: Documented byte ceiling for collected icons.

## Success Criteria

- **SC-001**: The reference/rename/comment/missing fixture collects exactly the referenced
  set with zero phantom assets.
- **SC-002**: Two builds produce byte-identical bundles and an untouched manifest.
- **SC-003**: Overrides, deterministic ordering, and the size ceiling behave per acceptance
  in all tested cases.
- **SC-004**: Provenance records the pinned version and license.

## Assumptions

- Icon references keep the established `iconNode("id", "name")` / `Icon("id", "name")`
  call shapes; new call shapes are out of scope.
- `lucide-static` stays a Bun-installed build-time package; the collector reads it from
  `node_modules`.
- The `assets/icons/` override convention is unchanged.
- Icon payloads are SVGs shipped verbatim; no rasterization or optimization passes.
