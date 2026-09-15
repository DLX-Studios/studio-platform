# Contract: Reload Protocol

**Status**: Contract for feature 005. Additive-only after landing.

## Transport

Unix `SOCK_STREAM` at `<project>/build/.studio-reload.sock`, newline-delimited JSON,
UTF-8. CLI awaits outcomes up to 30 s.

## Messages

```json
{"protocol":"studio-reload/1","type":"reload","request_id":"r-1","bundle":"/abs/path.studio","base_revision":3}
{"protocol":"studio-reload/1","type":"reloaded","request_id":"r-1","revision":4}
{"protocol":"studio-reload/1","type":"rejected","request_id":"r-1","stage":"validate","code":"MANIFEST_INVALID_JSON","message":"..."}
{"protocol":"studio-reload/1","type":"restart","request_id":"r-2"}
{"protocol":"studio-reload/1","type":"restarted","request_id":"r-2","revision":5}
```

- Unknown `type` → connection-level error, session unaffected.
- Protocol mismatch → rejection with stage `validate`, code `RELOAD_PROTOCOL_MISMATCH`.
- Every outcome echoes `request_id`; the CLI matches superseded requests by id and drops
  stale outcomes.

## Guarantees

- Single-flight: at most one swap executes; extra requests collapse to one trailing reload
  with the latest bundle.
- Never-dead-window: only committed swaps replace the live surface; all failures keep the
  last-good surface (or safe empty state) with a diagnostic.
- Revision advances exactly once per accepted reload or restart.
- State: route preserved-if-declared-else-default; instance always fresh.
- Trust: dev channel only; production admission untouched.
