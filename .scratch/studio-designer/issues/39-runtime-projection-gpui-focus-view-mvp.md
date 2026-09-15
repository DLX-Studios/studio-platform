# 39 [D]: Runtime Projection v0, GPUI app skeleton, Focus View MVP

**What to build:** Deterministic projection from an immutable Studio Design revision plus Library snapshot into protocol-compatible UiNode trees preserving source identities. GPUI desktop application frame hosting the canvas-first Focus View: open project, click-select nodes on canvas, edit properties via inspector commands, see undo work live. Preview mounts projection output through the retained registry and native renderers.

**Blocked by:** 23, 31, 37

**Status:** partial — deterministic Focus/projection acceptance passes; native Wayland smoke remains unavailable here

- [x] Identical revision produces byte-identical projection output
- [x] Canvas selection maps bidirectionally with hierarchy identities
- [x] Inspector edit flows command→revision→projection→visible repaint
- [x] Undo visibly reverts the last canvas action
- [x] Source-linked diagnostic appears when projecting an invalid construct

## Closure audit at `3a05109`

- [x] Identical revision produces byte-identical projection output — `crates/studio-design/src/projection.rs:928-975` compares repeated roots and serialized bytes.
- [x] Canvas selection maps bidirectionally with hierarchy identities — `crates/studio-design/src/projection.rs:404-515` preserves source IDs, `crates/studio-design/tests/canvas_manipulation.rs:347-377` covers hierarchy identity, and `crates/studio-designer/tests/focus_view.rs:109-115` covers Focus selection.
- [x] Inspector edit flows command→revision→projection→visible repaint — `crates/studio-designer/tests/focus_view.rs:109-137`; accepted outcomes refresh in `crates/studio-designer/src/focus_view.rs:1132-1150`.
- [x] Undo visibly reverts the last canvas action — session-backed undo/reprojection is exercised in `crates/studio-designer/tests/focus_view.rs:109-137` and the canvas manipulation undo path is implemented in `crates/studio-design/src/engine.rs:324-443`.
- [x] Source-linked diagnostic appears for invalid projection — `crates/studio-design/src/projection.rs:978-1005` asserts node-linked asset/composition diagnostics, surfaced as `ProjectionFailed` by `crates/studio-designer/src/focus_view.rs:162-181`.

Verdict: partial. Deterministic projection and Focus model evidence pass, but native Wayland compositor/input smoke is unavailable in this environment and remains an external verification gap.
