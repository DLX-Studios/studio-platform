# 24 [R]: Application users, roles, and row-scoped authorization

**What to build:** Generated applications support their own users — employee PIN/badge entry, email/password — verified entirely host-side. Roles bind to routes, screens, actions, and individual data records; row-scoped access is enforced at the data layer so bypassing the UI gains nothing.

**Blocked by:** 15

**Status:** host RBAC/PIN/row-scope service implemented/tested; complete guest/application integration remains open

- [ ] A technician-scoped principal requesting another employee's tickets is denied host-side, including through direct collection-helper calls
- [ ] PIN verification works fully offline against the app's user store
- [ ] Role and membership changes are auditable operations
- [ ] Repeated failed verification applies the declared throttle/lockout policy
- [ ] Enforcement has no dependency on interface visibility

## Completion audit — 2026-11-01

Current source/evidence assessment: F11 in the [project completion audit](../../../docs/development/PROJECT_COMPLETION_AUDIT.md). The status above supersedes earlier code-only/closed/readiness wording. Existing checkboxes and older evidence describe their original test layer; they do not certify an unexecuted native or live-service journey. Remaining work is in the [ordered backlog](../completion-tasks.md).
