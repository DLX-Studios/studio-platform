# 15 [R]: Application data namespaces and typed collection helpers

**What to build:** Each verified publisher/application pair receives an isolated Application Data Namespace. Applications read and write data only through typed host-mediated collection helpers (select/create/update-merge/update-patch/delete verbs) validated against declared schemas; guests, agents, MCP clients, and extensions never touch a database handle or query language.

**Blocked by:** 14

**Status:** ready-for-agent

- [ ] Namespace derives from signed publisher/app identity; two applications cannot observe each other's records through any guest interface
- [ ] Helpers cover full CRUD plus listing, enforcing declared record schemas
- [ ] Namespace/database switching or raw query attempts from guests fail closed with stable safe codes
- [ ] The point-of-sale example persists catalog and cart across restart using the helpers
- [ ] Throughput on the supported baseline machine meets a recorded preliminary budget
