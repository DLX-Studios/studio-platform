# 40 [D]: Canvas direct manipulation and hierarchy panel

**What to build:** Hit testing, drag, resize handles, reorder, reparent, duplicate, delete, restore — all as reversible named-undo-group batches. Snapping, guides, alignment, distribution. Selection stays stable across rename/move. Hierarchy panel backed by stable identities mirrors and edits the tree.

**Blocked by:** 39

**Status:** done

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

Verdict: closed. Domain and Focus control evidence pass; no code gap was found.
