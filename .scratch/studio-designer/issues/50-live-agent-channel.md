# 50 [D]: Live agent editing channel through the command engine

**What to build:** Host-mediated agent sessions with scoped reads (selection, subtree, schemas, diagnostics, history) and streamed typed command batches carrying actor attribution, base revision, preconditions, progress, and a per-batch undo-group identity. Cancellation stops future batches at boundary. User edits continue concurrently; overlapping stale batches resolve last-writer-wins at batch granularity, with the losing batch's intent preserved as diagnostics only (per the #08 override). Agents verify batches with `studio check` and self-correct from structured failures before completion.

**Blocked by:** 37, 22

**Status:** partial — channel model exists; accepted conflict/undo semantics and real native/checker/transport integration remain open

- [ ] Interleaved human and agent edits both land when independent
- [ ] Overlapping stale agent batch resolves last-writer-wins at batch granularity (per #08 override; losing batch surfaces diagnostics only)
- [ ] Each accepted batch undoes independently (per-batch undo; no cross-batch task grouping)
- [ ] Cancellation prevents acceptance of subsequent batches only
- [ ] Check failures flow back as machine-readable feedback enabling self-correction
- [ ] Progress, accepted operations, warnings, and failures visible live in the editor dock

## Completion audit — 2026-11-01

Current source/evidence assessment: F06/F07 in the [project completion audit](../../../docs/development/PROJECT_COMPLETION_AUDIT.md). The status above supersedes earlier code-only/closed/readiness wording. Existing checkboxes and older evidence describe their original test layer; they do not certify an unexecuted native or live-service journey. Remaining work is in the [ordered backlog](../completion-tasks.md).
