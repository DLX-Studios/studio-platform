# 30 [R]: Signed update channel with staged rollout and rollback

**What to build:** Deployed applications receive updates through a signed channel supporting staged rollout, health checks, and automatic rollback. Updating many installations is one channel operation.

**Blocked by:** 17, 18

**Status:** ready-for-agent

- [ ] Signature verification rejects tampered updates
- [ ] Staged rollout updates a configured fraction of instances before broadening
- [ ] A failing health check triggers automatic rollback to the prior version
- [ ] Update state and history are visible per installation
