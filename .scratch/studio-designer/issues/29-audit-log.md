# 29 [R]: Application audit log

**What to build:** An append-only log records security-relevant application events: authentication attempts, role and membership changes, destructive actions, data exports, webhook admissions, and workflow runs. Queryable in the Designer, exportable by the owner, redacted under standard rules.

**Blocked by:** 15

**Status:** host append/query/export/integrity service implemented/tested; producer/native query integration remains open

- [ ] Every listed event class is recorded when it occurs
- [ ] Tampering with the log is detectable
- [ ] Export produces complete but redacted records
- [ ] Query by time range, event type, and actor works at useful scale

## Completion audit — 2026-11-01

Current source/evidence assessment: F09/F11 in the [project completion audit](../../../docs/development/PROJECT_COMPLETION_AUDIT.md). The status above supersedes earlier code-only/closed/readiness wording. Existing checkboxes and older evidence describe their original test layer; they do not certify an unexecuted native or live-service journey. Remaining work is in the [ordered backlog](../completion-tasks.md).
