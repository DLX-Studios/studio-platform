# 49 [D]: Content collections, bindings, fixtures, and typed forms

**What to build:** Schema-aware content collections with CRUD authoring; typed repeated-content bindings rendering Library collections; fixture states (empty/loading/error/populated/edge) switchable in preview; declarative forms with validation.

**Blocked by:** 38, 48

**Status:** partial — session-backed content/forms APIs tested; native panels and authored-output integration remain open

- [ ] List bound to collection renders identically across all fixture states
- [ ] Binding type mismatch is a build error unless a valid fallback is declared
- [ ] Form validation runs declaratively in prototype mode
- [ ] Collection schema change surfaces affected bindings as diagnostics

## Completion audit — 2026-11-01

Current source/evidence assessment: F05/F08 in the [project completion audit](../../../docs/development/PROJECT_COMPLETION_AUDIT.md). The status above supersedes earlier code-only/closed/readiness wording. Existing checkboxes and older evidence describe their original test layer; they do not certify an unexecuted native or live-service journey. Remaining work is in the [ordered backlog](../completion-tasks.md).
