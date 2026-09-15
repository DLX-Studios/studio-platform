# Embedded SurrealDB topology for Studio Designer

Research snapshot: 2026-08-23. This note evaluates SurrealDB Rust SDK 3.2.4 and the current local Studio repositories. It uses only first-party SurrealDB documentation/source materials and local project source.

## Decision summary

Studio Designer can embed SurrealDB as its desktop persistence engine, but SurrealDB does **not** supply a supported embedded-database-to-hosted-database replication layer. Studio Designer must own synchronization as a domain-level protocol.

The recommended topology is:

```text
first-party Studio Designer host
  |
  +-- LocalStore (authoritative while editing/offline)
  |     embedded SurrealDB 3.2.4
  |     RocksDB-backed application-data directory
  |     materialized design state + operation outbox + sync cursors
  |
  +-- content-addressed local asset store
  |
  +-- SyncWorker (optional, authenticated, retryable)
        HTTPS/WSS to Studio Cloud sync API
          |
          +-- hosted SurrealDB
          |     accepted operation log + snapshots + account/device cursors
          |
          +-- object storage for asset blobs
```

Use RocksDB for the first shipping desktop engine. Keep the local store behind a narrow adapter and run a separate SurrealKV qualification spike; do not make v1 durability depend on SurrealKV while SurrealDB still labels it beta. The official deployment guidance describes RocksDB as the mature persistent option and SurrealKV as a beta engine aimed at embedded/local-first workloads whose smaller resident memory and simpler operation make it the path to evaluate first, not yet the conservative production choice ([deployment models](https://surrealdb.com/docs/manage/self-hosted/deployment-models)).

The desktop should not connect to hosted SurrealDB with a system username/password. It should authenticate to a Studio Cloud sync API; that service owns database credentials, authorization, canonical ordering, and conflict admission. This follows the existing Canvas pattern: its server process owns the remote Surreal connection and environment credentials, while its collaboration layer submits idempotent, base-revisioned operations ([Canvas Surreal connection](../../../../Studio-Canvas/studio-canvas-app/src/lib/server/cloud/surreal.ts), [Canvas collaboration store](../../../../Studio-Canvas/studio-canvas-app/src/lib/server/sync/collaboration.ts), [Canvas canonical commit](../../../../Studio-Canvas/studio-canvas-app/src/lib/server/cloud/canonical-workspace.ts)).

## What Rust embedding actually supports

The Rust SDK can run the complete query layer in-process and persist to a directory. Current feature flags include `kv-mem`, `kv-rocksdb`, `kv-surrealkv`, and `kv-tikv`; remote support is separately feature-gated by `protocol-ws` and `protocol-http` ([crate 3.2.4 features](https://docs.rs/crate/surrealdb/3.2.4/features)). The dynamic `engine::any::connect` API can select an embedded or remote endpoint at runtime, including `rocksdb://`, `surrealkv://`, `ws://`, and `https://` endpoints ([Rust `Any` connection API](https://docs.rs/surrealdb/3.2.4/surrealdb/engine/any/fn.connect.html)).

Relevant engine choices are:

| Engine | Supported use | Studio Designer judgment |
|---|---|---|
| `Mem` / SurrealMX | Volatile tests, scratch state; persistence/versioning modes also exist in 3.x | Use only for unit and model tests, never as the project authority. |
| `RocksDb` | Persistent embedded directory; mature local disk engine | Ship first. Accept native build/packaging cost. |
| `SurrealKv` | Persistent embedded directory, versioning, simpler embedded operation | Prototype and benchmark behind the adapter; currently beta. |
| `TiKV` | Distributed external store | Not an embedded desktop/offline topology. |

The SDK's embedded module warns that RocksDB and TiKV depend on non-Rust libraries, which adds host build prerequisites and compile time; it explicitly suggests the in-memory engine during development to avoid those costs ([embedded Rust module](https://docs.rs/surrealdb/3.2.4/surrealdb/engine/local/index.html)). Studio Platform currently targets Rust 1.93, while the current SDK requires Rust 1.89 or newer, so the language toolchain is compatible ([workspace toolchain](../../../Cargo.toml), [Rust SDK requirements](https://surrealdb.com/docs/reference/rust)).

For the production dependency, pin an exact SDK release and disable unused defaults. The local persistence crate should enable only the chosen embedded backend and required query features; remote synchronization should live in a different adapter so enabling a local database does not implicitly create a network path. Tests may feature-gate `kv-mem`. Studio Platform already exact-pins security-sensitive runtime dependencies, so an exact SurrealDB pin matches the repository's dependency policy ([workspace dependencies](../../../Cargo.toml)).

## Replication and synchronization feasibility

SurrealDB's replication is a property of distributed storage deployments. Its deployment matrix describes embedded databases as “application-bound,” while multi-node deployments obtain replication, consensus, and fault tolerance from the distributed storage layer ([deployment models](https://surrealdb.com/docs/manage/self-hosted/deployment-models)). An embedded RocksDB directory and a hosted database are two independent databases. Pointing two SDK clients at them does not link their transaction histories.

SurrealDB offers two useful primitives, neither of which is bidirectional offline synchronization:

- Live queries push current server mutations to an online client; the documentation directs consumers to changefeeds for replay/catch-up ([live queries](https://surrealdb.com/docs/learn/querying/real-time/live-queries)).
- `CHANGEFEED` plus `SHOW CHANGES` produces an ordered, retained stream with monotonically increasing backend versionstamps ([`SHOW CHANGES`](https://surrealdb.com/docs/reference/query-language/statements/show)). Its cursor belongs to the database that produced it and its retention can expire. It does not define cross-database identity, conflict resolution, access ownership, or client acknowledgement.

Therefore v1 synchronization should use an explicit Studio Design operation protocol, not storage-engine replication and not raw changefeed replay:

1. Every accepted local edit transaction atomically updates the local materialized design and appends an immutable outbox operation.
2. An operation carries at least `operation_id`, `device_id`, `project_id`, `actor_id`, `base_revision`, ordered command payload, schema/protocol version, content hashes, and creation time. `operation_id` is the idempotency key.
3. The sync worker sends unacknowledged operations in local order. The cloud service validates identity, project ownership, schema version, preconditions, and command invariants in one hosted transaction.
4. The service stores an immutable accepted-operation record, assigns a monotonically increasing per-project server revision, updates a snapshot/materialized view, and returns an idempotent receipt.
5. The client pulls accepted operations after its per-project server cursor, applies each through the same Design command engine, and updates its cursor atomically with the materialized state.
6. Independent operations may be rebased only by explicit operation-algebra rules. A same-property edit, delete-versus-edit, invalid parent/child change, or failed structural precondition becomes a first-class conflict; no last-writer-wins fallback silently discards work.
7. Periodic snapshots bound replay time. The operation log remains the audit and synchronization truth; snapshots are accelerators.

This develops the useful parts of Canvas's current contract—base revisions, idempotency receipts, audit records, recovery points, and explicit conflicts—without inheriting its snapshot-as-operation payload. Canvas currently queues an operation whose payload contains a full canonical snapshot, compares `baseRevision`, and records the base/current/proposed snapshots on conflict ([collaboration store](../../../../Studio-Canvas/studio-canvas-app/src/lib/server/sync/collaboration.ts), [canonical workspace transaction](../../../../Studio-Canvas/studio-canvas-app/src/lib/server/cloud/canonical-workspace.ts)). Studio Designer's typed node operations make finer merging possible, so full snapshots should be checkpoints rather than every sync message.

Cloud sync is optional. A local-only project never emits its outbox. Enabling cloud sync creates a cloud project identity and begins upload from a declared local revision; disabling it stops transfer but does not disable local editing. For same-user multi-device sync, the cloud-accepted sequence is canonical between devices, while each device remains authoritative for unsent offline work until it is accepted, rebased, or surfaced as a conflict.

## Process, trust, and security constraints

The embedded database belongs exclusively to the first-party host. Studio's accepted system architecture says the Rust host owns persistence, secrets, networking, and filesystem access; Wasm guests receive neither sockets nor file descriptors and have no WASI ([Studio system architecture](../../../docs/architecture/ADR-0001-system-boundaries.md)). Consequently:

- Generated Runtime applications, extensions, agents, and MCP clients never receive a Surreal handle or arbitrary SurrealQL access. They use validated host command interfaces.
- The sync worker is host-owned and is the only local component allowed to transmit project data.
- Hosted SurrealDB system credentials remain server-side. The desktop stores a short-lived Studio account token in an OS credential facility; raw credentials and tokens do not enter design records, the operation log, diagnostics, or backups.
- The local database directory and asset store live under the per-user application-data directory with owner-only permissions. They must not live in an extension-visible workspace or exported runtime bundle.
- Configure the embedded query engine deny-first. SurrealDB supports limiting scripting, functions, guest execution, and network targets through `Config`/`Capabilities`; its guidance recommends denying all production capabilities and allowing only what is needed ([capabilities](https://surrealdb.com/docs/learn/security/authorization/capabilities), [Rust capability type](https://docs.rs/surrealdb/3.2.4/surrealdb/opt/capabilities/struct.Capabilities.html)). Studio Designer does not need database-side JavaScript or database-initiated network requests.
- Treat any agent-authored value as bound data or a typed command field, never concatenate it into SurrealQL. The host's Design command validator is still authoritative even when the database schema is schemafull.

Embedding removes a listening database server but does not make local data confidential. SurrealDB's security guidance recommends OS/disk encryption such as LUKS or BitLocker for encryption at rest and notes that datastore-level encryption is backend-dependent, not supplied by the query layer ([security best practices](https://surrealdb.com/docs/learn/security/best-practices/security-best-practices)). V1 should require supported OS full-disk encryption for its documented threat model or add a separately evaluated encrypted storage layer. Studio Library blobs may contain private reference media, so backups and content-addressed files require the same protection as the database.

Large icons, images, audio, and video should remain content-addressed files/object-storage objects. Store hashes, metadata, bindings, provenance, and sync state in SurrealDB. This prevents the database file from becoming the transfer unit for large media and lets sync deduplicate and resume blobs independently from design operations.

## Durability, backup, and recovery constraints

The local store must be opened once by the host and participate in ordered startup/shutdown. Before an application update or datastore migration, stop editing and synchronization, create a recovery artifact, migrate, validate, then resume. Do not copy a live database directory as though it were a portable backup.

SurrealDB recommends portable logical exports plus storage snapshots where available; logical exports are engine-independent, while filesystem snapshots depend on layout and binary compatibility ([backups and recovery](https://surrealdb.com/docs/manage/self-hosted/backups-and-recovery)). For Studio Designer:

- Make periodic logical project snapshots in the Studio Design interchange representation, separate from the database's physical files.
- Maintain an operation journal/outbox sufficient to reconstruct changes since the latest snapshot.
- On upgrade, preserve the pre-migration local database directory or a logical export until post-migration validation succeeds.
- Test restore into a clean application-data directory, validate schema version and content hashes, replay operations, then launch the editor.
- Never include account secrets in a project backup. A restored project must reauthenticate before cloud sync resumes.

The Rust embedded connection exposes durability controls including syncing every commit or at an interval. Critical design operation + outbox transactions should choose an explicit durability mode and verify crash recovery; the SDK documents `SyncMode::Every` as the most durable and an interval as faster but less durable ([Rust connection options](https://docs.rs/surrealdb/3.2.4/surrealdb/struct.Connect.html)). The actual choice requires a performance/crash-recovery benchmark on Studio's supported hardware, but it cannot remain an implicit default.

## Schema and engine migration constraints

There are two distinct migrations:

1. **Studio schema migration:** changes to Design, operation, library, account, or cursor tables. Use checked-in, numbered, forward-only migrations with an application schema-version record. Run them transactionally where supported, with data invariants and a post-migration validator. Remote changes use expand/cutover/contract so old clients remain valid during rollout. SurrealKit's official rollout model follows that phased approach for shared databases ([SurrealKit rollouts](https://surrealdb.com/docs/manage/schema-migration/rollouts)).
2. **SurrealDB engine/on-disk migration:** changes caused by upgrading SurrealDB itself. Pin local and hosted versions deliberately; read release notes and rehearse against production-shaped copies. SurrealDB warns that major upgrades can change on-disk formats and may require `surreal fix` or logical export/import ([upgrades and patching](https://surrealdb.com/docs/manage/self-hosted/upgrades-and-patching), [major-version migration overview](https://surrealdb.com/docs/build/migrating/from-old-surrealdb-versions/overview)). The desktop updater must never open the only copy of a user database with a new major engine before creating a recoverable pre-upgrade copy.

Compile-time embedded schema files are supported by SurrealKit, but its `sync` behavior can prune definitions removed from the source. For user data, use it only with pruning disabled or as a schema bootstrap; destructive/data-transforming evolution stays in reviewed numbered migrations ([`embed_schema!`](https://surrealdb.com/docs/manage/schema-migration/embed-schema-macro)).

Operations and snapshots must carry their Studio schema/protocol version independently of the database engine version. A cloud service may reject a client that is too old to safely rebase, but must leave its local outbox intact and return an actionable upgrade status.

## Packaging and licensing constraints

RocksDB adds native C/C++ build and distribution work. The SurrealDB Rust docs explicitly warn that RocksDB depends on non-Rust libraries ([embedded Rust module](https://docs.rs/surrealdb/3.2.4/surrealdb/engine/local/index.html)). Before fixing it as the release backend, CI must prove a reproducible build, clean-machine launch, database create/open/reopen, crash recovery, backup/restore, and package-size/RSS/startup thresholds for every supported desktop target. The present Studio Platform target is native Wayland, so that is the first qualification target, not evidence for future macOS or Windows support ([Studio system architecture](../../../docs/architecture/ADR-0001-system-boundaries.md)).

SurrealDB core 3.0 is Business Source License 1.1, not Apache-2.0. Its Additional Use Grant permits embedding and redistributing SurrealDB in an application but prohibits offering a “Database Service” that lets third parties create, manage, or control schemas or tables; that release changes to Apache-2.0 on 2030-01-01 ([SurrealDB 3.0 license](https://github.com/surrealdb/surrealdb/blob/main/LICENSE), [official licensing explanation](https://github.com/surrealdb/license)). Studio Designer and its sync service fit the stated permitted application use only if they expose design/content functionality rather than a general database service. Before a commercial release:

- obtain a license review against the exact pinned SurrealDB/core/transitive versions;
- ship required BSL and third-party notices and generate an SBOM;
- ensure cloud APIs do not expose schema/table administration as a customer feature;
- repeat the review when upgrading the SurrealDB major/minor line or changing the cloud product shape.

## Required follow-on decisions and proofs

This research clears the sync and trust tickets to decide domain semantics, but implementation should not begin without these explicit proofs:

1. A RocksDB embedding spike in a disposable crate measuring clean/release build time, binary/package size, idle and edit-load RSS, startup/open latency, 10,000-operation replay, forced-process crash recovery, and logical restore.
2. A matching SurrealKV spike to determine whether its embedded advantages justify accepting beta risk later. It is an evaluation candidate, not the v1 default.
3. A two-device sync simulation covering duplicate delivery, reordering, disconnect during upload, accepted-but-response-lost, stale base revision, same-property conflict, structural conflict, deletion, asset upload resume, schema skew, and account revocation.
4. A migration rehearsal from the initially pinned engine/schema to one newer patch and one deliberately incompatible fixture, proving that the updater retains a recoverable original.
5. A license/security review confirming the hosted service is application sync rather than DBaaS, and confirming the local-at-rest threat model.

The architecture decision that can now be taken is: **embed SurrealDB with RocksDB behind a host-owned `LocalStore`; synchronize explicit Studio Design operations through a Studio Cloud service into hosted SurrealDB; never treat SurrealDB storage replication, changefeeds, or physical database files as the sync protocol.**
