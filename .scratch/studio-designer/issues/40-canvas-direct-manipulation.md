# 40 [D]: Canvas direct manipulation and hierarchy panel

**What to build:** Hit testing, drag, resize handles, reorder, reparent, duplicate, delete, restore — all as reversible named-undo-group batches. Snapping, guides, alignment, distribution. Selection stays stable across rename/move. Hierarchy panel backed by stable identities mirrors and edits the tree.

**Blocked by:** 39

**Status:** partial — gesture algebra and action controls implemented; direct pointer/hierarchy/native parity acceptance remains open

- [x] Full gesture set reversible in one named undo group per gesture
- [x] Guides snap within declared tolerance; alignment/distribution operate on multi-selection
- [x] Reparent preserves visual position intent
- [x] Hierarchy drag reorders identically to canvas gestures
- [x] Keyboard nudge/resize matches pointer behavior

## Closure audit at `3a05109`

- [x] Full gesture set is represented as named, reversible batches — builders and named groups: `crates/studio-design/src/manipulation.rs:810-1131`; inverse application is validated by `crates/studio-design/src/engine.rs:324-443`; end-to-end gesture journey: `crates/studio-design/tests/canvas_manipulation.rs:211-343`.
- [x] Guides snap within tolerance; alignment/distribution operate on multi-selection — `crates/studio-design/tests/canvas_manipulation.rs:170-208` and builders at `crates/studio-design/src/manipulation.rs:1102-1131`.
- [x] Reparent preserves visual position intent — `crates/studio-design/src/manipulation.rs:936-986` emits the unchanged frame after move.
- [x] Hierarchy drag/reorder uses the same command builders — `crates/studio-design/tests/canvas_manipulation.rs:346-377` and `crates/studio-design/src/manipulation.rs:1259-1280`.
- [x] Keyboard nudge/resize shares pointer algebra — `crates/studio-design/src/manipulation.rs:839-895`; Focus keyboard controls are wired in `crates/studio-designer/src/focus_view.rs:2512-2642`.

Historical closure verdict superseded by the 2026-11-01 completion audit; domain/model evidence alone does not close native acceptance.

## Completion audit — 2026-11-01

Current source/evidence assessment: F02/F08 in the [project completion audit](../../../docs/development/PROJECT_COMPLETION_AUDIT.md). The status above supersedes earlier code-only/closed/readiness wording. Existing checkboxes and older evidence describe their original test layer; they do not certify an unexecuted native or live-service journey. Remaining work is in the [ordered backlog](../completion-tasks.md).
