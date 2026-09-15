# 41 [D]: Flow layout and overlay placement authoring

**What to build:** Authoring of row/column/grid/stack containers with padding, gaps, sizing, and constraints through inspector and canvas handles; explicit overlay/absolute placement inside supported stacks so badges/chips sit over media using ordinary primitives.

**Blocked by:** 40

**Status:** done

- [x] Badge-over-image composed from primitives with typed placement properties only
- [x] Invalid layout combinations are explained at the selected node
- [x] Constraint changes respond correctly across device-profile switches

## Closure audit at `3a05109`

- [x] Badge-over-image uses ordinary primitives with typed placement — `crates/studio-design/tests/layout_overlay.rs:144-184` and typed constructors in `crates/studio-design/src/model.rs:503-582`.
- [x] Invalid layout combinations explain the selected node — `crates/studio-design/tests/layout_overlay.rs:186-225`; diagnostics always carry node identity in `crates/studio-design/src/engine.rs:4602-4848`.
- [x] Constraint changes respond across profile switches — `crates/studio-design/tests/layout_overlay.rs:227-335` exercises sparse merge, undo/redo, and profile switching without changing revision.

Verdict: closed. Layout, overlay diagnostics, and responsive constraint evidence pass; no code gap was found.
