# 27 [R]: Declarative scheduled and event-triggered workflows

**What to build:** Time, interval, and event triggers execute bounded typed actions against application state and plugins, running wherever the declared data topology keeps authoritative state. Workflow definitions participate in validation, diagnostics, and audit like any other declarative artifact.

**Blocked by:** 15, 23

**Status:** ready-for-agent

- [ ] Interval and fixed-time triggers fire correctly including clock drift and missed-fire policy
- [ ] Event triggers respond to declared application and webhook events
- [ ] A failing run isolates failure and applies its declared retry policy without corrupting state
- [ ] Runs appear in the audit log with actor workflow-identity
