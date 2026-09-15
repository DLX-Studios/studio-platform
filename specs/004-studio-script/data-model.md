# Data Model: Studio Script and Embedded Development Host

**Date**: 2026-09-03 · Entities for [spec.md](spec.md) · mirrors §6 of
`docs/STUDIO_SCRIPT_TRANSFORM_PIPELINE.md`.

## StudioModule

One compiled `.studio` file.

| Field | Type | Notes |
|-------|------|-------|
| `id` | ModuleId | Stable, derived from module identity (path + content role) |
| `source` | SourceFile | Path, fingerprint, language version |
| `imports` | list\<Import\> | Static imports resolved through the module graph |
| `exports` | list\<Export\> | Component names exposed to importers |
| `components` | list\<ComponentDefinition\> | Ordered by source |
| `dependencies` | list\<ModuleId\> | Transitive closure for invalidation |

Validation rules: no duplicate component names; no import cycles (module graph rejects with the
cycle named); every import resolves to a known module or reserved virtual module.

## ComponentDefinition

| Field | Type | Notes |
|-------|------|-------|
| `id` | ComponentId | Stable: module identity + structural/source identity |
| `name` | string | Matches the declaration site |
| `props` | list\<PropDefinition\> | Typed, with defaults; `name`/`type`/`required`/`default` |
| `state` | list\<StateSlot\> | `$state` declarations in source order |
| `derived` | list\<DerivedSlot\> | `$derived` recomputed from dependencies |
| `handlers` | list\<EventHandler\> | Event → behavior bindings |
| `template` | list\<TemplateNode\> | Root template forest |
| `span` | SourceSpan | Declaration span for diagnostics |

## StateSlot / DerivedSlot

| Field | Type | Notes |
|-------|------|-------|
| `id` | StateSlotId / DerivedSlotId | Stable per slot |
| `name` | string | Source binding name |
| `ty` | StudioType | Closed type; compatibility checked on swap |
| `initial` | Expression | State initializer snapshot |
| `dependencies` | list\<SlotId\> | Derived slots only |

Swap rule: preserve a state slot only when stable ID **and** `StudioType` match; otherwise
dispose and initialize fresh. There is one canonical numeric type, so numeric edits never
count as type changes.

## TemplateNode (closed)

`Component { id, kind, explicit_key?, props, children, span }` ·
`Text { id, value, span }` ·
`Interpolation { id, expression, span }` ·
`If { id, condition, consequent, alternate, span }` ·
`Each { id, collection, item, index?, key, body, fallback, span }` (fallback renders for an empty collection).

Validation rules: kinds are closed catalog kinds; `Each` requires a keyed array collection;
keys are unique among siblings; IDs are stable per §6 rules.

## Expression (closed)

`Literal` · `ReadProp` · `ReadState` · `ReadDerived` · `ReadLocal` · `Unary` · `Binary` ·
`Conditional` · `Array` · `Record` · `CallApprovedFunction`. No raw source text is executable.

## StudioType (closed)

Primitives (including one canonical `number`), arrays, maps, records, closed SDK types.
Language→host numeric mapping: integral values in i64 range → Integer, all other numbers →
Decimal, applied deterministically by the validator; exact-decimal rendering uses explicit
formatting helpers.

## ModuleGraph

Directed import graph across `.studio` modules: cycle detection with the cycle named, static
import resolution (including reserved virtual modules), dependency-ordered invalidation.

## ArtifactFingerprint

Versioned cache key over: source contents, resolved dependency graph, language/IR version
(`STUDIO_IR_VERSION` = 2), SDK and generated-binding versions, `rsvelte` revision (from
`EngineFingerprint`), ASC version and options, target ABI and host protocol version.
Identical fingerprints yield byte-identical artifacts.

## SwapTransaction

Atomic old→new module replacement: outcome is accepted (with per-slot migration report:
preserved / initialized / disposed) or rejected (last-valid keeps running); rollback on host
validation failure.
