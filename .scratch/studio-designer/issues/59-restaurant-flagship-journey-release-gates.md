# 59: Flagship restaurant operations journey and release evidence gates

**What to build:** Extend POS into the multi-terminal restaurant proof: employee roles with PIN login; shared table/check/ticket state across at least three stations plus kitchen display via center topology; offline service window with reconciliation; hourly tracking feeding payroll export; split/single/per-seat billing; receipt and kitchen printing through real peripheral adapters; Stripe sandbox payment through the REST broker; an agent-assisted authoring step demonstrating per-batch undo (one undo group per accepted batch). Then run the full release-blocking evidence suites.

**Blocked by:** 24, 25, 27, 29, 30, 33, 34, 49, 51, 53, 55

**Status:** deterministic flagship harness implemented/tested; physical/live/manual release gates remain blocking

- [ ] Demo-day script passes end-to-end on baseline hardware with three physical stations
- [ ] Offline window: orders taken disconnected reconcile exactly once after reconnect
- [ ] Payroll export matches tracked hours; billing flows split correctly under concurrent edits
- [ ] Stripe sandbox payment completes via declared REST routes; secrets never appear anywhere observable
- [ ] Capability matrix fully certified — zero fallback rendering in shipped journey
- [ ] Audit log captures the complete scenario; determinism/recovery/security/a11y suites green within budgets
- [ ] Every gate that cannot pass is recorded as an explicit waiver with a named owner; no silent skips

## Completion audit — 2026-11-01

Current source/evidence assessment: F14/F15 in the [project completion audit](../../../docs/development/PROJECT_COMPLETION_AUDIT.md). The status above supersedes earlier code-only/closed/readiness wording. Existing checkboxes and older evidence describe their original test layer; they do not certify an unexecuted native or live-service journey. Remaining work is in the [ordered backlog](../completion-tasks.md).
