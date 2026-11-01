# 27 [R]: Declarative scheduled and event-triggered workflows

**What to build:** Time, interval, and event triggers execute bounded typed actions against application state and plugins, running wherever the declared data topology keeps authoritative state. Workflow definitions participate in validation, diagnostics, and audit like any other declarative artifact.

**Blocked by:** 15, 23

**Status:** workflow scheduler implemented/tested; product/runtime orchestration integration remains open

- [ ] Interval and fixed-time triggers fire correctly including clock drift and missed-fire policy
- [ ] Event triggers respond to declared application and webhook events
- [ ] A failing run isolates failure and applies its declared retry policy without corrupting state
- [ ] Runs appear in the audit log with actor workflow-identity

## Completion audit — 2026-11-01

Current source/evidence assessment: F11 in the [project completion audit](../../../docs/development/PROJECT_COMPLETION_AUDIT.md). The status above supersedes earlier code-only/closed/readiness wording. Existing checkboxes and older evidence describe their original test layer; they do not certify an unexecuted native or live-service journey. Remaining work is in the [ordered backlog](../completion-tasks.md).
