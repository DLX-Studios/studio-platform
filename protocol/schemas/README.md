# Generated Schemas

This directory contains checked-in JSON Schemas generated from authoritative Rust types in
`crates/studio-protocol`. Do not hand-edit generated schema files. Regenerate them through the
repository protocol-generation command and commit the Rust source and generated delta together.

Protocol fixtures live under `protocol/fixtures/` and are checked by both Rust and AssemblyScript
tests.

## Protocol v1 inventory

`protocol-v1/` must contain these deterministic Rust-authoritative JSON Schemas:

- `guest-message.schema.json`
- `host-event.schema.json`
- `mount-tree.schema.json`
- `patch-batch.schema.json`
- `navigation-command.schema.json`
- `action-request.schema.json`
- `action-result.schema.json`

Every document uses JSON Schema draft 2020-12, has a stable Studio `$id`, and is serialized with a
single trailing newline. Run `cargo run -p studio-protocol --bin generate_schema` to regenerate
the complete inventory atomically.
