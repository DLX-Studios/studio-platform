# 53 [D]: Plugin/template installation UX and generated settings surfaces

**What to build:** Browse and install integration plugins and vertical templates inside Designer; settings rendered automatically as top-level tab groups of nested typed fields (text/number/boolean/color/image/select/secret-reference/device-picker) from plugin-declared schemas; template brand slots map to tokens so customers rebrand by swapping token values; ingestion import review showing sources, inferred entities, warnings, and destination before tracked commands apply.

**Blocked by:** 35, 50

**Status:** partial — tracked install/settings APIs tested; native browse/install/settings surfaces remain open

- [ ] Template instantiation applies ordinary tracked commands, undoable as one group
- [ ] Rebranding swaps brand tokens only; layout untouched
- [ ] Every settings field type renders generically; changes are tracked commands
- [ ] Import review blocks execution until reviewed; applied nodes retain source provenance

## Completion audit — 2026-11-01

Current source/evidence assessment: F08/F12 in the [project completion audit](../../../docs/development/PROJECT_COMPLETION_AUDIT.md). The status above supersedes earlier code-only/closed/readiness wording. Existing checkboxes and older evidence describe their original test layer; they do not certify an unexecuted native or live-service journey. Remaining work is in the [ordered backlog](../completion-tasks.md).
