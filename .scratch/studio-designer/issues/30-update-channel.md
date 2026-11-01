# 30 [R]: Signed update channel with staged rollout and rollback

**What to build:** Deployed applications receive updates through a signed channel supporting staged rollout, health checks, and automatic rollback. Updating many installations is one channel operation.

**Blocked by:** 17, 18

**Status:** signed channel/rollout/rollback logic implemented/tested; deployment and native installation history remain open

- [ ] Signature verification rejects tampered updates
- [ ] Staged rollout updates a configured fraction of instances before broadening
- [ ] A failing health check triggers automatic rollback to the prior version
- [ ] Update state and history are visible per installation

## Completion audit — 2026-11-01

Current source/evidence assessment: F15 in the [project completion audit](../../../docs/development/PROJECT_COMPLETION_AUDIT.md). The status above supersedes earlier code-only/closed/readiness wording. Existing checkboxes and older evidence describe their original test layer; they do not certify an unexecuted native or live-service journey. Remaining work is in the [ordered backlog](../completion-tasks.md).
