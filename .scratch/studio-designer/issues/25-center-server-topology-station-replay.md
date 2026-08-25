# 25 [R]: Center-server topology and station offline replay

**What to build:** Multi-station applications declare a center holding shared operational truth, reachable through one protocol whether deployed as a self-hosted on-premises hub or a Studio Cloud-hosted namespace. Stations enroll via pairing tokens; reads and writes resolve against the center; during disconnection stations queue writes locally and replay with explicit conflict preservation.

**Blocked by:** 15, 19

**Status:** ready-for-agent

- [ ] Two stations plus a hub converge identical shared table/check state
- [ ] Writes queued during a disconnect replay exactly once logically after reconnect
- [ ] Conflicting concurrent writes preserve both intents as resolvable conflicts rather than last-writer-wins
- [ ] Hub restart loses no acknowledged operation
- [ ] Station-local storage holds only settings and cache, never operational truth
