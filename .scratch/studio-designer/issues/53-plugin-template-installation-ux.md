# 53 [D]: Plugin/template installation UX and generated settings surfaces

**What to build:** Browse and install integration plugins and vertical templates inside Designer; settings rendered automatically as top-level tab groups of nested typed fields (text/number/boolean/color/image/select/secret-reference/device-picker) from plugin-declared schemas; template brand slots map to tokens so customers rebrand by swapping token values; ingestion import review showing sources, inferred entities, warnings, and destination before tracked commands apply.

**Blocked by:** 35, 50

**Status:** ready-for-agent

- [ ] Template instantiation applies ordinary tracked commands, undoable as one group
- [ ] Rebranding swaps brand tokens only; layout untouched
- [ ] Every settings field type renders generically; changes are tracked commands
- [ ] Import review blocks execution until reviewed; applied nodes retain source provenance
