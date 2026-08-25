# 28 [R]: Development, staging, and production environment isolation

**What to build:** Application data and secret values are isolated per environment. Packages reference secret names; active-environment configuration supplies values. Promoting between environments never moves credential material.

**Blocked by:** 18

**Status:** ready-for-agent

- [ ] Three environments coexist on one machine with independent data stores
- [ ] Secret resolution follows the active environment, not the package
- [ ] Promotion copies no secret material and proves it
- [ ] Cross-environment access attempts produce safe diagnostics
