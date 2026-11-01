# 20 [R]: WebSocket session broker

**What to build:** Applications exchange real-time messages through host-owned sessions admitted by signed declarations constraining endpoint, subprotocol, message schemas, sizes, rates, lifetime, and lifecycle events. Guests hold opaque session identities and typed events, never sockets.

**Blocked by:** 19

**Status:** host broker implemented/tested; generic guest event delivery and live WSS verification remain open

- [x] Session open/close/error lifecycle events reach the guest as typed inputs
- [x] Inbound and outbound messages validate against declared schemas
- [x] Reconnect behavior belongs to the host per declaration; guests cannot reconnect independently
- [x] Rate and size limits enforce mid-session, not just at open
- [ ] Integration suite exercises an approved real WebSocket endpoint

## Verification notes (2026-09-25)

- `cargo test --locked -p studio-net`: passed, including the lifecycle, schema, reconnect, mid-session size, and rate checks.
- `cargo clippy --locked -p studio-net --all-targets --all-features -- -D warnings`: passed, including the operator-gated WSS test target.
- The real endpoint test was not run: `STUDIO_NET_REAL_WEBSOCKET_URL` is absent. Its staging contract and command are documented in `docs/net/STAGING_TRANSPORT.md`.
- Full-workspace tests/Clippy remain blocked by missing `libclang.so` while building `surrealdb-librocksdb-sys`.

## Completion audit — 2026-11-01

Current source/evidence assessment: F11/F14 in the [project completion audit](../../../docs/development/PROJECT_COMPLETION_AUDIT.md). The status above supersedes earlier code-only/closed/readiness wording. Existing checkboxes and older evidence describe their original test layer; they do not certify an unexecuted native or live-service journey. Remaining work is in the [ordered backlog](../completion-tasks.md).
