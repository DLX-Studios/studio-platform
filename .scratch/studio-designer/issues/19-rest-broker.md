# 19 [R]: REST request broker with declared schemas and streaming

**What to build:** Applications perform HTTP only through a host-owned broker governed by signed route-group declarations constraining origins, methods, paths, headers, response schemas, and generous explicit size/rate/timeout limits. Route groups declare a credential source: public, an OAuth provider-plugin session, or a named secret reference injected host-side. Declared routes support server-sent-event streaming responses delivering validated typed chunk events with cancellation and host-owned reconnect policy.

**Blocked by:** 18

**Status:** ready-for-agent

- [ ] Requests to undeclared origins or paths are denied with stable codes
- [ ] Responses failing declared schema validation never reach guest memory
- [ ] Credential values appear in neither guest memory nor any log/diagnostic surface
- [ ] Streaming route delivers incremental validated chunks, honors cancellation, and applies declared bounds
- [ ] Integration suite executes against an approved real endpoint, not a simulator
