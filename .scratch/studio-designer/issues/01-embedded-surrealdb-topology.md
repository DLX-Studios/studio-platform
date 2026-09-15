# Determine the embedded SurrealDB topology

Type: research
Status: resolved

## Question

What supported embedded SurrealDB architecture can provide Studio Designer's local offline authority while synchronizing optionally with a hosted SurrealDB service, and what operational, packaging, licensing, security, migration, and conflict constraints must later architecture decisions account for?

## Answer

Embed SurrealDB 3.2.4 with the mature RocksDB backend behind a host-owned `LocalStore`. Keep the local materialized design, immutable operation outbox, sync cursors, and metadata in that store; keep large Studio Library media in a content-addressed local asset store. Use a separate authenticated `SyncWorker` and Studio Cloud API to exchange typed Studio Design operations with hosted SurrealDB. SurrealDB has no supported embedded-to-hosted replication protocol, so neither storage replication, physical database files, nor raw changefeed replay can be the product sync protocol.

Each accepted local operation must atomically update materialized state and append an idempotent outbox record. The cloud service validates identity, schema, project ownership, base revision, and command invariants, assigns a per-project server revision, stores the accepted operation, and returns an idempotent receipt. Clients pull accepted operations by cursor; same-property, structural, deletion, or schema conflicts become explicit recoverable conflicts. Periodic logical Studio Design snapshots bound replay time.

The local database is host-only; guests, extensions, agents, and MCP clients receive typed commands, never a Surreal handle or SurrealQL. Hosted credentials remain server-side. Startup/shutdown, crash recovery, logical backup, engine/schema migrations, exact dependency pinning, RocksDB packaging, and the SurrealDB BSL 1.1 license require dedicated tests and release review. SurrealKV remains a qualification spike, not the v1 durability default.

See [embedded-surrealdb-topology.md](../research/embedded-surrealdb-topology.md) for primary-source citations and the required qualification proofs.
