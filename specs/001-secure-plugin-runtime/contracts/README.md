# Milestone-One Contracts

These documents define the planning-level external contracts for protocol version 1. Runtime
implementation must encode them as closed Rust types in `studio-protocol`; generated JSON Schema,
AssemblyScript bindings, fixtures, and SDK documentation must be checked against that authority.

| Contract | Scope |
| --- | --- |
| [host-guest-v1.md](host-guest-v1.md) | ABI, envelopes, UI, patches, navigation, events, lifecycle |
| [bundle-v1.md](bundle-v1.md) | `.studio` archive, manifest, signing input, validation order |
| [actions-v1.md](actions-v1.md) | capabilities, payment, opaque references, receipts, printing |

## Compatibility Rules

- `protocolVersion: 1` and `schemaVersion: 1` are exact major contract selectors.
- Unknown fields, node kinds, properties, message variants, capabilities, operations, imports, and
  exports are rejected.
- A breaking change requires a new protocol/schema version, compatibility fixtures, migration
  notes, and an explicit rollout plan.
- Generated artifacts are committed and CI fails if regeneration produces a diff.
- Limits in these contracts are ceilings; a manifest may request a lower limit but never a higher
  one without a future host contract.
- Error messages are diagnostic and may evolve; stable error codes and observable state changes
  are contractual.

## Security Ownership

The Rust host owns validation, native UI, focus, accessibility, navigation, animation, secrets,
confirmation, capabilities, filesystem, networking, devices, storage, and shutdown. A valid
signature establishes publisher identity, not trust in plugin messages or behavior.
