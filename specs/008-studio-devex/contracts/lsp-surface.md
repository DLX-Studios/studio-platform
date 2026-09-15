# Contract: Language Server Protocol Surface

**Status**: Contract for feature 008. Additive-only after landing.

The server speaks JSON-RPC over stdio with these methods (params/results as JSON):

- `studio/format`: `{uri, source}` → `{formatted} | {diagnostic}`. Idempotent.
- `studio/diagnostics`: `{uri}` → `{diagnostics: [{code, message, line, column}]}` ordered.
- `studio/complete`: `{uri, line, character}` → `{items: [{label, kind, documentation}]}`.
- `studio/hover`: `{uri, line, character}` → `{contents} | null`.
- `studio/definition`: `{uri, line, character}` → `{uri, line, character} | null`.
- `studio/rename`: `{uri, line, character, newName}` → `{edits} | {unavailable, reason}`.
- `studio/references`: `{uri, line, character}` → `{locations: [...]}`.
- `studio/shutdown`: `{}` → `{ok: true}`.

All positions are 1-based line/character to match Studio spans. Failures never
crash the server; malformed requests get error responses. Upstream gaps degrade
to `{unavailable, reason}` or reported ranges, never silent behavior.
