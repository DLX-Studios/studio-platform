# 38 [D]: Command families II — responsive, tokens, bindings, interactions, compositions

**What to build:** Remaining command families through the same engine: base-plus-breakpoint responsive values, design-token definition/application, typed content bindings, declarative interactions graph, and Reusable Composition define/instance/override semantics including identity-retaining propagation from definition updates.

**Blocked by:** 37

**Status:** ready-for-agent

- [ ] Each family demoable through public seam queries and CLI replay
- [ ] Composition instance reflects definition change while retaining instance identity
- [ ] Overrides admitted only where the composition contract allows
- [ ] Stale-precondition submissions return structured conflicts, never silent overwrite
- [ ] Unknown fields in any closed schema are rejected at decode
