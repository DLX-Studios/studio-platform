# 57 [D]: Cloud Identity onboarding and Designer synchronization UX

*Cloud/conflict semantics in issue 09 are resolved; real service/native integration and acceptance remain open.*

**What to build:** Cloud register → email verify/resend/confirm → personal workspace setup → completion to dashboard, with recoverable failure states throughout. Expired/offline states explained without hiding cached projects. Sync enable/disable reversible; disable stops transfer while preserving local authority and unsent operations. Consistent offline/connecting/synced/warning/error indicators across auth, dashboard, and project shell.

**Blocked by:** 55, resolution of grilling issue 09

**Status:** partial — onboarding/sync models exist; real cloud service and native flow remain blocked on integration/provisioning

- [ ] End-to-end registration succeeds against real test identity service
- [ ] Disabling sync halts transfers; re-enabling resumes without loss
- [ ] Cached projects remain fully usable offline after cloud sign-out

## Completion audit — 2026-11-01

Current source/evidence assessment: F09/F13 in the [project completion audit](../../../docs/development/PROJECT_COMPLETION_AUDIT.md). The status above supersedes earlier code-only/closed/readiness wording. Existing checkboxes and older evidence describe their original test layer; they do not certify an unexecuted native or live-service journey. Remaining work is in the [ordered backlog](../completion-tasks.md).
