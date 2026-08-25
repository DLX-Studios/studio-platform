# 26 [R]: Inbound webhook listeners

**What to build:** Applications declare inbound endpoints received by host-owned listeners constrained by signed declarations covering endpoint identity, payload schemas, source verification, sizes, rates, and lifetime. Validated events route into the application as ordinary typed inputs.

**Blocked by:** 19

**Status:** ready-for-agent

- [ ] Unsigned, malformed, oversized, and flooded requests are rejected with safe codes before reaching guest logic
- [ ] Valid payloads arrive as schema-validated typed events
- [ ] Guest code cannot bind ports or observe the listener directly
- [ ] Admission of each delivery is recorded for audit
