# 26 [R]: Inbound webhook listeners

**What to build:** Applications declare inbound endpoints received by host-owned listeners constrained by signed declarations covering endpoint identity, payload schemas, source verification, sizes, rates, and lifetime. Validated events route into the application as ordinary typed inputs.

**Blocked by:** 19

**Status:** webhook broker implemented/tested; production listener/guest-delivery integration remains open

- [ ] Unsigned, malformed, oversized, and flooded requests are rejected with safe codes before reaching guest logic
- [ ] Valid payloads arrive as schema-validated typed events
- [ ] Guest code cannot bind ports or observe the listener directly
- [ ] Admission of each delivery is recorded for audit

## Completion audit — 2026-11-01

Current source/evidence assessment: F11 in the [project completion audit](../../../docs/development/PROJECT_COMPLETION_AUDIT.md). The status above supersedes earlier code-only/closed/readiness wording. Existing checkboxes and older evidence describe their original test layer; they do not certify an unexecuted native or live-service journey. Remaining work is in the [ordered backlog](../completion-tasks.md).
