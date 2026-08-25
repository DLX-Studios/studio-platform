# 17 [R]: Signed application data migrations

**What to build:** Signed data migrations execute in a dedicated pre-launch lifecycle with a created recovery point, version checks, idempotent application, and a rollback/recovery policy. Migrations always complete before ordinary guest access begins.

**Blocked by:** 14

**Status:** ready-for-agent

- [ ] Schema v1 to v2 rehearsal includes a crash injected mid-migration and ends in a usable prior-or-new state, never a half-migrated store
- [ ] Recovery point is created before execution and restorable
- [ ] Failed migration quarantines the application with a safe diagnostic instead of launching
- [ ] Engine upgrades remain a separately rehearsed path from schema migrations
