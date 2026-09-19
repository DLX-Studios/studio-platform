# Define live agent and MCP editing semantics

Type: grilling
Status: resolved
Blocked by: 03, 04

## Question

How should agents and MCP read scoped context, stream validated operation batches into the active design, expose progress and diagnostics, group history, support cancellation and undo, recover from failures, and coexist with user edits without a proposal gate?

## Answer

Agent and MCP edits apply live through the same validated command engine as user edits, with no proposal gate. Sessions get scoped reads (selection, subtree, schemas, diagnostics, history) and stream typed command batches carrying actor attribution, base revision, and preconditions. Batches verify with `studio check`; failures flow back machine-readable for self-correction. Progress, accepted operations, warnings, and failures are visible live in the editor dock.

Concurrency is last-writer-wins at batch granularity (user override of #50's structured-conflict draft). Overlapping stale agent batches overwrite at the batch boundary; no conflict preservation beyond the losing batch's diagnostics. Each accepted batch undoes independently (per-batch undo, no cross-batch task grouping). Cancellation stops acceptance of subsequent batches only; already-accepted batches remain and undo individually.

See [CONTEXT.md](../../CONTEXT.md) for the settled glossary.
