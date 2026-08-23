# Invalid protocol-v1 fixtures

This directory is the checked-in inventory for hostile protocol fixtures. Generation in T016 will
materialize JSON examples covering:

- unknown envelope fields, payload fields, variants, node kinds, and operations;
- unsupported protocol versions;
- empty, oversized, and duplicate node IDs;
- node-count, tree-depth, message-size, and patch-operation budget excess;
- zero, replayed, and decreasing patch sequences;
- relative or malformed routes and closed navigation variants;
- guest-supplied `owner` fields on messages, operations, and host-context UI events;
- malformed action results and lifecycle variants.

The Rust negative suite in `crates/studio-protocol/tests/protocol_v1.rs` is authoritative until the
cross-language fixture generator lands.
