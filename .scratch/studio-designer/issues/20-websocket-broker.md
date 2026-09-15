# 20 [R]: WebSocket session broker

**What to build:** Applications exchange real-time messages through host-owned sessions admitted by signed declarations constraining endpoint, subprotocol, message schemas, sizes, rates, lifetime, and lifecycle events. Guests hold opaque session identities and typed events, never sockets.

**Blocked by:** 19

**Status:** ready-for-agent

- [ ] Session open/close/error lifecycle events reach the guest as typed inputs
- [ ] Inbound and outbound messages validate against declared schemas
- [ ] Reconnect behavior belongs to the host per declaration; guests cannot reconnect independently
- [ ] Rate and size limits enforce mid-session, not just at open
- [ ] Integration suite exercises an approved real WebSocket endpoint
