# 49 [D]: Content collections, bindings, fixtures, and typed forms

**What to build:** Schema-aware content collections with CRUD authoring; typed repeated-content bindings rendering Library collections; fixture states (empty/loading/error/populated/edge) switchable in preview; declarative forms with validation.

**Blocked by:** 38, 48

**Status:** ready-for-agent

- [ ] List bound to collection renders identically across all fixture states
- [ ] Binding type mismatch is a build error unless a valid fallback is declared
- [ ] Form validation runs declaratively in prototype mode
- [ ] Collection schema change surfaces affected bindings as diagnostics
