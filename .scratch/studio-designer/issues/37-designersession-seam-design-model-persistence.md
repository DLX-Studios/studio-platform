# 37 [D]: DesignerSession seam, Studio Design model, and persistence

**What to build:** The deep `DesignerSession` interface — typed queries and command batches in; immutable snapshots, command receipts, diagnostics, and conflict results out. Studio Design source model: flat stable-ID node map with parent index, screens, compositions, tokens, responsive variants, typed interaction references. Structural and property command families commit as atomic batches producing immutable revisions and named undo groups; undo applies validated inverses as new revisions. Persistence through the LocalStore with crash recovery.

**Blocked by:** 14, 22

**Status:** implemented-code-only (awaiting serialized runner/fixer pass)

- [x] Seam tests cover create/open/edit/undo/redo/reopen without touching internals
- [x] Invalid batch rolls back atomically leaving no new revision
- [x] Node identities survive rename, move, reorder, and styling edits
- [x] Deletion produces tombstone information sufficient for undo and reference diagnostics
- [x] No GPUI, SurrealDB, or Runtime UiNode types appear in the session interface
- [x] Kill after a durable point recovers the last accepted revision

## Implementation notes

Branch `tt/37-designersession-seam`. The implementation is intentionally split into three commits:
the public model/seams, the command/history engine, and the host persistence adapter. No Cargo
command was run in this writer worktree; the serialized runner still owns the locked workspace
test, Clippy, format, and release-build gates.

### Interface and source model

- New dependency-light `studio-design` crate. It owns the closed version-1 Studio Design schemas,
  opaque stable IDs, flat node map and parent index, screens, Reusable Compositions, tokens,
  responsive variants, typed interactions, revision metadata, tombstones, diagnostics, command
  batches, receipts, conflicts, and session context. Serde records use `deny_unknown_fields`.
- `DesignerSession` is object-safe and host-independent: typed queries and command/history requests
  enter; owned snapshots, receipts, diagnostics, and structured conflicts leave. GPUI, SurrealDB,
  cloud transport, and Runtime `UiNode` types do not cross the seam.
- `DesignerPersistence` is owned by the domain crate and speaks only in versioned domain
  transactions. `studio-host` depends inward on `studio-design`; the domain crate has no host or
  database dependency.

### Commands, revisions, and history

- Structural/property families implemented: insert, move, reorder, explicit-ID-map duplicate,
  delete, exact tombstone restore, set/remove property, and rename. A batch mutates only a cloned
  snapshot, then validates the complete flat-map/parent-index invariants before it can commit.
- Every accepted operation creates a monotonically increasing immutable revision and deterministic
  receipt. Stale bases and structural/property preconditions return conflicts. Invalid commands
  return diagnostics and leave both live and durable state unchanged.
- Inverses are captured from prior typed values and placements. Contiguous batches sharing a named
  undo-group identity undo together; undo/redo apply validated commands as new revisions rather
  than moving storage backward. Tombstones retain the full subtree, indexed parents, detached
  placement, deletion revision, and typed reference diagnostics.
- Public seam tests cover create/open/edit/undo/redo/reopen, rollback, identity stability,
  duplication, tombstone restore/diagnostics, grouped undo, stale conflicts, and closed-schema
  decode. These 8 pure domain tests were compiled, linted (`all` + `pedantic` + `-D warnings`), and
  executed directly with `rustc`/`clippy-driver`; all passed without invoking Cargo.

### LocalStore persistence and recovery

- `LocalStoreDesignerPersistence` lives in `studio-host`, wraps only `EmbeddedLocalStore` configured
  with `Durability::Every`, and replaces one deterministic per-project record atomically. The
  record contains the current snapshot, every immutable revision, accepted receipts/batches,
  history/inverses, diagnostics, and cursor, so reopen reconstructs the last complete transaction.
- Real RocksDB integration coverage creates, edits, undoes, redoes, closes, reopens, and queries a
  session only through public seams. A separate worker reaches an accepted Designer durable point,
  signals the parent, is force-killed, and the recovery process asserts that exact revision and
  property value reopen.

### UNVERIFIED items (for the runner/fixer pass)

1. UNVERIFIED(compile) — the new `studio-host -> studio-design` lockfile edge and host adapter/tests
   were authored against the landed LocalStore API but not compiled through Cargo in this writer.
2. UNVERIFIED(runtime) — SurrealDB/RocksDB serialization of the versioned Designer envelope through
   `StoreBatchEntry.payload` and forced-termination recovery are asserted by real integration tests
   but require the serialized runner to execute them.
3. UNVERIFIED(scale) — v1 atomically replaces the complete per-project revision/history envelope on
   each accepted batch. This is deliberately simple and crash-safe for the current seam; measure
   large-project/revision histories before selecting a later append/checkpoint compaction scheme.
