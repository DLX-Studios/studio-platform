# 14 [R]: Embedded LocalStore foundation

**What to build:** Any first-party host process can create, open, use, and recover an embedded SurrealDB store backed by RocksDB in a chosen directory, with explicit durability configuration. Establishes the async-storage boundary with the GPUI executor so no later feature improvises one.

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

- [ ] Store initializes schema metadata and reopens idempotently across process restarts
- [ ] Forced termination mid-write recovers to the last durable transaction with no partially committed batch
- [ ] Durability configurations are documented and covered by tests
- [ ] Storage calls run off the UI thread through a defined async boundary
- [ ] Deterministic tests pass against temporary RocksDB directories and the in-memory engine
- [ ] Incompatible or corrupted engine fixtures fail safely with an actionable, safe-coded diagnostic
