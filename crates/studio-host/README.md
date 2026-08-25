# studio-host

`studio-host` owns first-party process boundaries: embedded persistence, Wasmtime capability
bridging, package loading, security, hardware, and rendering. Existing crates remain compatibility
implementations until each vertical slice is migrated and tested.

## LocalStore

`LocalStore` is the only embedded Designer persistence surface. It uses exactly
`surrealdb = "=3.2.4"` with RocksDB and keeps the engine and SurrealQL private. Callers receive
typed batches and safe diagnostics; guests, extensions, agents, and MCP clients never receive a
SurrealDB handle or arbitrary-query capability.

Open it at a host-selected, owner-only application-data directory and select durability explicitly:

| Mode | Meaning | Suitable use |
| --- | --- | --- |
| `Durability::Every` | Syncs every committed transaction before success. | Accepted Designer edits; required crash-safe mode. |
| `Durability::Interval(duration)` | Flushes periodically; accepted writes after the last flush can be lost. | Benchmarks and disposable development data only. Duration must exceed 100 ms. |
| `Durability::Never` | Defers flushing to the operating system; acknowledged writes can be lost. | In-memory-style tests and disposable data only. |

Use `StoreExecutor` to submit owned `Send + 'static` operations from GPUI rather than opening or
writing on a UI callback. GPUI state must not cross the boundary. During shutdown, stop new work,
await every `StoreTask`, close each `LocalStore`, then call `StoreExecutor::shutdown`.

The host maintains a versioned engine manifest and schema-metadata record. Unsupported or damaged
fixtures return stable diagnostic codes and recovery guidance, never raw RocksDB/SurrealDB errors.
Logical project snapshots and operation journals—not live RocksDB directories—remain the portable
recovery format.

## Release review flags

RocksDB builds and packages native C/C++ code. Release CI must qualify clean-machine builds,
create/open/reopen, forced-termination recovery, logical restore, and package footprint for every
supported desktop target.

SurrealDB 3.x is under BSL 1.1, not this workspace's Apache-2.0 license. Before release, review
the exact SurrealDB core and transitive versions, ship required notices/SBOM material, and confirm
the product remains an application rather than a customer-controlled database service. Repeat the
review for SurrealDB upgrades or material cloud-product changes.
