# 58 [D]: Conflict and recovery centers

*Cloud/conflict semantics in issue 09 are resolved; real service/native integration and acceptance remain open.*

**What to build:** Conflict center reachable from dashboard and project settings before opening a project: affected projects, safe status, required actions, resolution choices preserving both intents. Recovery/migration/quarantine surfaces for interrupted upgrades, failed restores, and recoverable projects backed by logical snapshots plus operation journals.

**Blocked by:** 57

**Status:** partial — conflict/recovery models and route helpers exist; native centers and real sync recovery remain open

- [ ] Seeded conflict appears pre-open with both intents visible
- [ ] Each resolution choice preserves the promised intent
- [ ] Restore from logical snapshot plus journal rebuilds a working project

## Completion audit — 2026-11-01

Current source/evidence assessment: F09/F13 in the [project completion audit](../../../docs/development/PROJECT_COMPLETION_AUDIT.md). The status above supersedes earlier code-only/closed/readiness wording. Existing checkboxes and older evidence describe their original test layer; they do not certify an unexecuted native or live-service journey. Remaining work is in the [ordered backlog](../completion-tasks.md).
