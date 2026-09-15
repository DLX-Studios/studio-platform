# 25 [R]: Center-server topology and station offline replay

**What to build:** Multi-station applications declare a center holding shared operational truth, reachable through one protocol whether deployed as a self-hosted on-premises hub or a Studio Cloud-hosted namespace. Stations enroll via pairing tokens; reads and writes resolve against the center; during disconnection stations queue writes locally and replay with explicit conflict preservation.

**Blocked by:** 15, 19

**Status:** implemented-pending-verification

- [ ] Two stations plus a hub converge identical shared table/check state
- [ ] Writes queued during a disconnect replay exactly once logically after reconnect
- [ ] Conflicting concurrent writes preserve both intents as resolvable conflicts rather than last-writer-wins
- [ ] Hub restart loses no acknowledged operation
- [ ] Station-local storage holds only settings and cache, never operational truth

## Implementation notes (UNVERIFIED)

`crates/studio-host/src/topology.rs` adds a transport-neutral, host-owned
`CenterServer` and `Station` seam. `CenterTopology` distinguishes a self-hosted
endpoint from a Studio Cloud namespace while keeping the protocol identical;
the implementation intentionally performs no real network I/O. Pairing tokens
are deterministic testable values, single-use, logically expiring, and stored
as digests. Enrollments are center- and station-scoped proofs whose credential
has no public accessor.

The center is authoritative for records, tombstones, revisions, operation
receipts, and conflict history. Operation IDs are fingerprinted for exactly-once
logical replay: retrying an acknowledged operation returns `Replayed` without
advancing state, while reusing an ID for another payload fails closed. A stale
write records both the authoritative record and incoming intent as an explicit
`CenterConflict`; only `resolve_conflict` can mark it resolved.

Stations expose only `StationLocalState` (settings plus a materialized cache).
Disconnected writes remain in a transient outbox, are replayed in order after
reconnect, and are removed only after a center result. `PersistentCenter` stores
one serialized center snapshot through the existing host `LocalStore` batch
boundary with `Durability::Every` determining acknowledged-write durability.

`crates/studio-host/tests/topology.rs` covers two-station convergence, pairing,
offline replay, local-state boundaries, conflict preservation/resolution, and
center restart replay. Integration with a production on-premises protocol
server or Studio Cloud API remains a follow-up for the network service layer;
the current branch deliberately proves the typed host seam in-process only.
