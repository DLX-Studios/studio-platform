# 41 [D]: Flow layout and overlay placement authoring

**What to build:** Authoring of row/column/grid/stack containers with padding, gaps, sizing, and constraints through inspector and canvas handles; explicit overlay/absolute placement inside supported stacks so badges/chips sit over media using ordinary primitives.

**Blocked by:** 40

**Status:** partial — layout/overlay command models implemented; actual native layout/preview parity remains open

- [x] Badge-over-image composed from primitives with typed placement properties only
- [x] Invalid layout combinations are explained at the selected node
- [x] Constraint changes respond correctly across device-profile switches

## Closure audit at `3a05109`

- [x] Badge-over-image uses ordinary primitives with typed placement — `crates/studio-design/tests/layout_overlay.rs:144-184` and typed constructors in `crates/studio-design/src/model.rs:503-582`.
- [x] Invalid layout combinations explain the selected node — `crates/studio-design/tests/layout_overlay.rs:186-225`; diagnostics always carry node identity in `crates/studio-design/src/engine.rs:4602-4848`.
- [x] Constraint changes respond across profile switches — `crates/studio-design/tests/layout_overlay.rs:227-335` exercises sparse merge, undo/redo, and profile switching without changing revision.

Historical closure verdict superseded by the 2026-11-01 completion audit; domain/model evidence alone does not close native acceptance.

## Completion audit — 2026-11-01

Current source/evidence assessment: F02 in the [project completion audit](../../../docs/development/PROJECT_COMPLETION_AUDIT.md). The status above supersedes earlier code-only/closed/readiness wording. Existing checkboxes and older evidence describe their original test layer; they do not certify an unexecuted native or live-service journey. Remaining work is in the [ordered backlog](../completion-tasks.md).
