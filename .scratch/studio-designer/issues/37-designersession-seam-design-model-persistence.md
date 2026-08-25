# 37 [D]: DesignerSession seam, Studio Design model, and persistence

**What to build:** The deep `DesignerSession` interface — typed queries and command batches in; immutable snapshots, command receipts, diagnostics, and conflict results out. Studio Design source model: flat stable-ID node map with parent index, screens, compositions, tokens, responsive variants, typed interaction references. Structural and property command families commit as atomic batches producing immutable revisions and named undo groups; undo applies validated inverses as new revisions. Persistence through the LocalStore with crash recovery.

**Blocked by:** 14, 22

**Status:** ready-for-agent

- [ ] Seam tests cover create/open/edit/undo/redo/reopen without touching internals
- [ ] Invalid batch rolls back atomically leaving no new revision
- [ ] Node identities survive rename, move, reorder, and styling edits
- [ ] Deletion produces tombstone information sufficient for undo and reference diagnostics
- [ ] No GPUI, SurrealDB, or Runtime UiNode types appear in the session interface
- [ ] Kill after a durable point recovers the last accepted revision
