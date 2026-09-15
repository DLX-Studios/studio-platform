# Component Platform Data Model

## Component Definition

| Field | Rule |
| --- | --- |
| kind | Closed protocol enum; unknown kinds rejected |
| stable_id | Instance-scoped bounded identifier |
| properties | Kind-specific closed values and limits |
| children | Kind-specific cardinality and depth rules |
| events | Typed, owner-checked event names |
| semantics | Label, role, enabled/value/selected state, focus policy |
| native_mapping | Host-owned implementation classification |

## Component Instance

An instance belongs to exactly one plugin instance and retains native identity, focus, scroll,
input, selection, and overlay state across valid targeted updates. Terminal plugin failure drops all
instance state.

## Overlay Instance

An overlay records owner, anchor or region, focus policy, dismissal policy, reduced-motion policy,
and lifecycle status. Pending protected actions may add a navigation guard.

## Catalog Contract

The Rust protocol enum and property validators are authoritative. JSON Schema, fixtures, and
AssemblyScript builders are generated artifacts and must remain drift-free.
