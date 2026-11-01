# 28 [R]: Development, staging, and production environment isolation

**What to build:** Application data and secret values are isolated per environment. Packages reference secret names; active-environment configuration supplies values. Promoting between environments never moves credential material.

**Blocked by:** 18

**Status:** host isolation models implemented/tested; complete deployed environment flow remains open

- [ ] Three environments coexist on one machine with independent data stores
- [ ] Secret resolution follows the active environment, not the package
- [ ] Promotion copies no secret material and proves it
- [ ] Cross-environment access attempts produce safe diagnostics

## Completion audit — 2026-11-01

Current source/evidence assessment: F11 in the [project completion audit](../../../docs/development/PROJECT_COMPLETION_AUDIT.md). The status above supersedes earlier code-only/closed/readiness wording. Existing checkboxes and older evidence describe their original test layer; they do not certify an unexecuted native or live-service journey. Remaining work is in the [ordered backlog](../completion-tasks.md).
