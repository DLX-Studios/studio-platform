# 29 [R]: Application audit log

**What to build:** An append-only log records security-relevant application events: authentication attempts, role and membership changes, destructive actions, data exports, webhook admissions, and workflow runs. Queryable in the Designer, exportable by the owner, redacted under standard rules.

**Blocked by:** 15

**Status:** ready-for-agent

- [ ] Every listed event class is recorded when it occurs
- [ ] Tampering with the log is detectable
- [ ] Export produces complete but redacted records
- [ ] Query by time range, event type, and actor works at useful scale
