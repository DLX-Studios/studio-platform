# Define full extension authority

Type: grilling
Status: resolved
Blocked by: 02, 03, 04, 06

## Question

How can sandboxed extensions fully contribute declarative modules, inspector surfaces, commands, actions, content types, validators, migrations, and capability-gated services while preserving Studio's closed protocol and reserving native component implementations for first-party Rust code?

## Answer

Extensions are sandboxed and declarative. Third-party extensions can never add native renderer kinds or raw GPUI access. Native component implementations remain first-party Rust. Extensions contribute Reusable Compositions, typed settings schemas, inspector surfaces, commands, actions, content types, validators, migrations, and capability-gated services through signed descriptors.

Every extension ships a signed descriptor (identity, publisher, version, compatibility, contributions, requested capabilities). Tampered or incompatible descriptors are rejected before activation. Capability requests require explicit per-project consent, are recorded per project, and are revocable.

Lifecycle hooks are bounded in time and memory; a failing hook is contained and never hangs the project. Removal reports remaining artifacts owned by the extension before changing the project.

See [CONTEXT.md](../../CONTEXT.md) for the settled glossary.
