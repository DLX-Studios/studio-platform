# 15 [R]: Application data namespaces and typed collection helpers

**What to build:** Each verified publisher/application pair receives an isolated Application Data Namespace. Applications read and write data only through typed host-mediated collection helpers (select/create/update-merge/update-patch/delete verbs) validated against declared schemas; guests, agents, MCP clients, and extensions never touch a database handle or query language.

**Blocked by:** 14

**Status:** implemented-code-only (awaiting serialized runner/fixer pass)

- [x] Namespace derives from signed publisher/app identity; two applications cannot observe each other's records through any guest interface
- [x] Helpers cover full CRUD plus listing, enforcing declared record schemas
- [x] Namespace/database switching or raw query attempts from guests fail closed with stable safe codes
- [x] The point-of-sale example persists catalog and cart across restart using the helpers
- [x] Throughput has a recorded preliminary budget for the supported baseline machine (measurement remains a runner task)

## Implementation notes

Kept in `crates/studio-host`: application data is a typed mediation layer over the already private
`LocalStore`, and a new crate would add an abstraction boundary without adding authority isolation.
`ApplicationDataNamespace` uses a closed v1, domain-separated, length-delimited SHA-256 derivation
over the verified stable publisher ID and application/plugin ID. Signing-key rotation, bundle
updates, process instances, and restarts therefore retain the same partition.

Each bound `ApplicationDataHandle` contains the derived namespace and verified package collection
declarations. Guest requests contain only select/list/create/update-merge/update-patch/delete
shapes. Raw query, namespace switch, and database switch attempts carry operation markers only—no
query or selector string reaches the host storage layer—and return stable safe codes. Collections
persist as namespace-derived LocalStore batches with a format/schema header and ordered records;
all values are validated before writes and again after reads.

Tests in `crates/studio-host/tests/application_data.rs` cover key rotation stability,
cross-application isolation and attributed cross-namespace denial, forbidden guest operations,
all declared-schema collection verbs, schema rejection, and POS-style catalog/cart state across a
clean store close/reopen. Per handoff, no Cargo command was run in this worktree.

### Preliminary performance budget (recorded, not enforced)

On the project's supported baseline machine with `Durability::Every`, a 1,000-record collection
should target p95 ≤ 10 ms for select, p95 ≤ 25 ms for create/update/delete, and p95 ≤ 75 ms for a
full ordered list after warmup. The serialized runner should record machine/storage details and at
least 1,000 sampled operations per verb before treating these figures as qualification evidence.
These are preliminary targets, not assertions in the automated suite.
