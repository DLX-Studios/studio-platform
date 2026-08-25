# 50 [D]: Live agent editing channel through the command engine

**What to build:** Host-mediated agent sessions with scoped reads (selection, subtree, schemas, diagnostics, history) and streamed typed command batches carrying actor attribution, base revision, preconditions, progress, and undo-group identity. Cancellation stops future batches at boundary. User edits continue concurrently; stale work returns structured conflicts preserving both intents. Agents verify batches with `studio check` and self-correct from structured failures before completion.

**Blocked by:** 37, 22

**Status:** ready-for-agent

- [ ] Interleaved human and agent edits both land when independent
- [ ] Overlapping stale agent batch returns structured conflict without losing either intent
- [ ] One task spanning many batches undoes as one named group
- [ ] Cancellation prevents acceptance of subsequent batches only
- [ ] Check failures flow back as machine-readable feedback enabling self-correction
- [ ] Progress, accepted operations, warnings, and failures visible live in the editor dock
