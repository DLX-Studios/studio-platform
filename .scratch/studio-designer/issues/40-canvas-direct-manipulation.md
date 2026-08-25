# 40 [D]: Canvas direct manipulation and hierarchy panel

**What to build:** Hit testing, drag, resize handles, reorder, reparent, duplicate, delete, restore — all as reversible named-undo-group batches. Snapping, guides, alignment, distribution. Selection stays stable across rename/move. Hierarchy panel backed by stable identities mirrors and edits the tree.

**Blocked by:** 39

**Status:** ready-for-agent

- [ ] Full gesture set reversible in one named undo group per gesture
- [ ] Guides snap within declared tolerance; alignment/distribution operate on multi-selection
- [ ] Reparent preserves visual position intent
- [ ] Hierarchy drag reorders identically to canvas gestures
- [ ] Keyboard nudge/resize matches pointer behavior
