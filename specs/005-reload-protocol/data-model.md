# Data Model: Runtime Reload and Preview Protocol

**Date**: 2026-09-03 · Entities for [spec.md](spec.md).

## ReloadRequest

| Field | Type | Notes |
|-------|------|-------|
| `protocol` | string | `"studio-reload/1"`; mismatches rejected |
| `type` | string | `"reload"` |
| `request_id` | string | Client-generated correlation id |
| `bundle` | path string | Candidate bundle to prepare |
| `base_revision` | u64 | Client's last known session revision |

## ReloadOutcome

`Reloaded { request_id, revision }` · `Rejected { request_id, stage, code, message }`
where `stage` ∈ `validate|instantiate|commit` and `code` reuses build diagnostic families
plus `RELOAD_*` for channel errors.

## RestartRequest / RestartOutcome

`{"type":"restart","request_id"}` → `{"type":"restarted","request_id","revision"}` or a
rejection. Dispose-then-instantiate ordering with exactly-once revision advance.

## SwapTransaction

`Preparing → Prepared | PrepareFailed` → `Committing → Committed | RolledBack`. Rollback
returns to the live surface; disposal happens only inside `Committed`.

## SessionRevision

u64 monotonic counter owned by the runtime; starts at 1 on first mount; advances exactly
once per accepted reload or restart.

## ChannelEndpoint

Socket path + liveness: bind (unlinking own stale file first), connect with refused-as-
stale semantics, remove on clean exit.
