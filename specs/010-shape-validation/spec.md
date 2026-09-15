# Feature Specification: Check-Time Shape Validation

**Feature Branch**: `010-shape-validation`

**Created**: 2026-09-06

**Status**: Draft

**Input**: `studio check` passes screens that fail at mount: unknown props
(`title` on `Card`), leaf nodes with element children (`Text` wrapping
components), and over-full single-child containers (`Card` with four
children) all surface only as host mount rejections without source spans.
Check should fail fast with file/line diagnostics from one shared rule
source.

## Clarifications

### Session 2026-09-06

- Q: Where do the rules live? → A: `studio-protocol` owns them next to the
  mount validator: `child_cardinality(kind)` (which `validate_child_count`
  is refactored onto, so runtime behavior cannot drift) and
  `is_known_property(kind, name)` (a name-only mirror of
  `validate_specific`, cross-checked by tests).
- Q: What about prop VALUES at check time? → A: Out of scope. Values need
  evaluation (expressions) or mount context; only names are checked.
  Value errors stay runtime errors.
- Q: What about dynamic children (`{#if}`/`{#each}`)? → A: Conservative.
  Count checks skip nodes with direct `If`/`Each` children (counts are
  unknowable); leaf-content rules still apply to static element children.
- Q: Which attribute names are exempt? → A: `id` (identity, never a mount
  prop), event attributes (`onclick`/`onchange`/`onsubmit`, owned by other
  rules), and anything already rejected elsewhere (`key`, directives,
  spreads) to avoid double diagnostics.

## User Scenarios & Testing

### User Story 1 - Typo a prop, check tells me the line (Priority: P1)

An author writes `<Card title="x">`. `studio check` fails with
`STUDIO015` naming `title`, the `Card` kind, and the source span. Today
this builds fine and dies at mount.

### User Story 2 - Overfill a card, check suggests the fix (Priority: P1)

An author nests four children in a `Card`. Check fails with `STUDIO015`
stating the single-child rule and suggesting a wrapping `Column`.

### User Story 3 - Existing projects keep passing (Priority: P1)

All current fixtures, examples, and the pos-clothing-store screen pass
`studio check` unchanged: no new false positives on valid trees.

## Functional Requirements

- FR-1: `studio-protocol` exposes `child_cardinality()` (`Any`/`AtMostOne`/
  `None`/`ExactlyOne`) and `is_known_property()`; `validate_child_count`
  is refactored onto the former with no behavior change.
- FR-2: The `.studio` validator checks every catalog component node:
  unknown mount props → `STUDIO015`; element children under `None` kinds →
  `STUDIO015`; over-full `AtMostOne`/`ExactlyOne` nodes with static
  children → `STUDIO015` with the wrapping hint.
- FR-3: New `STUDIO015` code (`CODE_SHAPE`), documented alongside the
  other codes; diagnostics carry node spans.
- FR-4: Cross-check tests assert the name mirror agrees with
  `validate_specific` on a per-kind corpus, and every blessed valid
  fixture still checks clean.

## Non-Goals

- Prop value validation at check time.
- Dynamic (`If`/`Each`) count inference.
- Changes to mount-time validation semantics.

## Success Criteria

- SC-1: The three user-story snippets fail `studio check` with `STUDIO015`
  and spans; all existing valid sources still pass.
- SC-2: Full workspace gate green.
