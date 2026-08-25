# 16 [R]: Bounded Surreal query capability

**What to build:** Applications that declare the capability in their signed manifest may run SurrealQL through the host with host-side parameter binding only, permanently locked to their own namespace. All other applications are unaffected.

**Blocked by:** 15

**Status:** ready-for-agent

- [ ] Parameters bind host-side; string interpolation into query text is impossible through the interface
- [ ] Namespace or database switching inside queries is rejected
- [ ] System data, filesystem, scripting, and network functions reachable from queries are denied
- [ ] Query size, result size, and duration limits are enforced and configurable per declaration
- [ ] Denied attempts produce safe, source-linked diagnostics
