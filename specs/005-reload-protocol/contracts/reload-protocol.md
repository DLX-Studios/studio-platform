# Contract: Reload Protocol

**Status**: Contract for feature 005. Additive-only after landing.

## Transport

Local filesystem `SOCK_STREAM`, newline-delimited UTF-8 JSON. CLI awaits outcomes up to 30 s.
Unix uses the standard-library domain socket at `<project>/build/.studio-reload.sock`,
falling back to a stable hashed temporary filename for deep checkout paths. Windows uses
AF_UNIX through the Windows-only `uds_windows` adapter and always uses that short temporary
filename. Both peers compute it with `studio_protocol::reload::socket_path`; the CLI passes
the result via `--reload-socket`. No TCP listener is opened. Windows requires AF_UNIX support
(Windows 10 version 1803 or later).

The 2026-11-01 platform adapter is additive: wire version `studio-reload/1`, message shapes,
request correlation, revision semantics, and Unix endpoint selection remain compatible.

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
