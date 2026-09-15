# 57 [D]: Cloud Identity onboarding and Designer synchronization UX

*Gated: blocked until grilling issue 09 resolves and the test identity service exists.*

**What to build:** Cloud register → email verify/resend/confirm → personal workspace setup → completion to dashboard, with recoverable failure states throughout. Expired/offline states explained without hiding cached projects. Sync enable/disable reversible; disable stops transfer while preserving local authority and unsent operations. Consistent offline/connecting/synced/warning/error indicators across auth, dashboard, and project shell.

**Blocked by:** 55, resolution of grilling issue 09

**Status:** blocked

- [ ] End-to-end registration succeeds against real test identity service
- [ ] Disabling sync halts transfers; re-enabling resumes without loss
- [ ] Cached projects remain fully usable offline after cloud sign-out
