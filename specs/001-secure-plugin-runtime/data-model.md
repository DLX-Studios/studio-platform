# Phase 1 Data Model: Secure Native Plugin Runtime

## Modeling Rules

- IDs are opaque, non-empty, bounded UTF-8 strings unless a stronger representation is stated.
- Plugin-local IDs are never globally trusted; host lookups always include the owning instance.
- Monetary values are signed 64-bit integer minor units plus an explicit supported currency code.
- Raw secret bytes exist only inside host-owned secret records and are never serialized.
- Public protocol types are closed and versioned. Unknown fields, variants, and transitions fail.
- State mutation occurs only after complete request validation and authorization.

## Plugin Bundle

An installable immutable archive evaluated before any guest code executes.

| Field | Type | Rules |
| --- | --- | --- |
| archive | bounded byte stream | At most 16 MiB before extraction |
| schema_version | u16 | Exactly 1 for this feature |
| plugin_id | string | Stable reverse-domain identity |
| display_name | string | Non-empty, bounded, safe for host UI |
| version | semantic version | Valid and compatible with host policy |
| publisher_id | string | Must match trust-store identity |
| publisher_key_id | string | Resolves to an enabled trusted public key |
| entry | relative path | Exactly the declared module, no traversal |
| sdk_requirement | version range | Compatible with the packaged SDK contract |
| protocol_version | u16 | Exactly one supported version |
| capabilities | set | Closed values; no duplicates or unknown entries |
| limits | BundleLimits | Within host ceilings; cannot weaken host limits |
| asset_index | ordered digest list | Unique normalized paths, total at most 1 MiB |
| module_digest | SHA-256 | Covers module bytes; module at most 8 MiB |
| signature | 64 raw bytes | Ed25519 signature over the RFC 8785 canonical digest document |
| bundle_digest | SHA-256 | Host-computed immutable bundle identity |

### Validation state

```text
received -> structure_valid -> digest_valid -> publisher_trusted
         -> compatible -> wasm_valid -> ready
any state -> rejected (terminal; no compile/instantiate after rejection)
```

Developer mode replaces only `publisher_trusted` with an explicit `development_untrusted` state.
All other validations and budgets remain identical.

## Plugin Principal

The complete security identity used for every host decision.

| Field | Type | Rules |
| --- | --- | --- |
| publisher_key_id | KeyId | From verified bundle or explicit dev identity |
| plugin_id | PluginId | From the closed manifest |
| bundle_digest | SHA-256 | Prevents silently changing content under one ID |
| instance_id | random 128-bit ID | New for every launch and manual restart |
| trust_mode | production/development | Development is always visibly untrusted |

Principals compare on every field. Matching plugin IDs do not permit cross-instance or
cross-bundle access.

## Plugin Instance

One isolated Wasmtime store and its host-owned state.

| Field | Type | Rules |
| --- | --- | --- |
| principal | PluginPrincipal | Immutable for the lifecycle |
| lifecycle | InstanceLifecycle | Host-controlled transition only |
| budgets | RuntimeBudgets | Host ceilings intersected with manifest requests |
| ui_registry | UIRegistry | Empty before a successful mount |
| navigation | NavigationStack | Owned by this instance, depth at most 32 |
| pending_actions | map<RequestId, ActionRequest> | At most 16 |
| owned_secrets | set<SecretHandleId> | References only, never secret bytes |
| last_patch_sequence | u64 | Enforces ordered non-replayed batches |
| diagnostic | optional stable failure | Redacted, safe for host surface |

### Lifecycle

```text
created -> loading -> running -> stopping -> stopped
                    |    |
                    |    +-> terminated(trap/resource/protocol)
                    +------> terminated(load/validation)

terminated -> [operator selects restart] -> new created instance
```

`terminated` is terminal. Restart never reuses the store, instance ID, UI registry, navigation
state, pending actions, handles, or guest memory. Host shutdown or compositor loss moves active
instances through `stopping`, cancels actions, revokes secrets, and does not create replacements.

## Runtime Budgets

| Budget | Host ceiling |
| --- | --- |
| Linear memory | 16 MiB, one memory |
| Tables | One bounded table |
| Fuel | 10,000,000 per init/event |
| Epoch deadline | 50 ms per guest call |
| Generic inbound event | 64 KiB |
| Initial mount | 1 MiB |
| Patch batch | 256 KiB and 512 operations |
| String property | 64 KiB |
| UI tree | 5,000 nodes, depth 64 |
| Node ID | 128 UTF-8 bytes |
| Pending actions | 16 |
| Navigation stack | 32 entries |

## UI Tree and Node

The host-retained representation of a validated mount.

| Field | Type | Rules |
| --- | --- | --- |
| owner | InstanceId | Required on every registry access |
| node_id | NodeId | Unique within the instance and stable until removal |
| kind | closed NodeKind | Must be supported by protocol version |
| properties | typed property map | Names/types allowed for the node kind only |
| parent | optional NodeId | Root has none; no cycles |
| children | ordered NodeId list | Cardinality allowed by node kind |
| event_bindings | typed binding map | Non-secret events only |
| native_state | host-private reference | Never crosses the protocol boundary |

The root and every inserted/replaced subtree must satisfy uniqueness, depth, count, property,
child-cardinality, event, and accessibility rules before commit.

## Update Batch

| Field | Type | Rules |
| --- | --- | --- |
| owner | InstanceId | Must match active instance |
| sequence | u64 | Strictly greater than last committed sequence |
| operations | ordered PatchOp list | 1–512 operations |
| encoded_size | bytes | At most 256 KiB |
| validation_result | pending/valid/rejected | Rejected batches never mutate the registry |

### Transaction state

```text
received -> decoded -> structurally_valid -> semantically_valid -> staged -> committed
any pre-commit state -> rejected (registry unchanged)
```

Validation includes the combined effect of all operations, so later operations may refer to valid
earlier staged inserts but cannot create duplicates, cycles, invalid indices, or excess budgets.

## Route Definition and Navigation Stack

### Route definition

| Field | Type | Rules |
| --- | --- | --- |
| pattern | absolute typed path | Static and named parameter segments only in v1 |
| screen_factory | lazy screen key | Instantiated only after a successful match |
| guard | optional guard key | Must resolve within 50 ms; timeout blocks |
| transition | approved transition policy | Host validates and honors reduced motion |

### Stack entry

| Field | Type | Rules |
| --- | --- | --- |
| entry_id | host ID | Unique within instance |
| route | canonical route | Must match a declared route or not-found route |
| params | typed string map | Derived from match, bounded |
| local_state | host/SDK screen state | Disposed when entry is removed |

Push, replace, pop, pop-to, and reset are atomic. A pending payment guard blocks or requests
host-owned confirmation before abandoning checkout.

## Capability Declaration and Decision

| Field | Type | Rules |
| --- | --- | --- |
| capability | enum | `payment.simulate` or `printer.simulate` only |
| manifest_declared | bool | Must be true |
| host_policy_allowed | bool | Must be true |
| principal | PluginPrincipal | Must match the requester |
| operation | closed operation | Must belong to capability |
| confirmation_requirement | policy | Payment requires trusted confirmation |

Unknown or undeclared capability requests fail without prompting.

## Action Request

| Field | Type | Rules |
| --- | --- | --- |
| request_id | RequestId | Unique among current/preserved idempotent records |
| owner | PluginPrincipal | Captured by host, not trusted from payload |
| capability | CapabilityId | Declared and authorized |
| operation | closed string | Valid for capability |
| payload | typed value | Fully validated before state transition |
| state | ActionState | Host-controlled |
| result | optional ActionResult | Redacted and correlated; retained until host exit for payments |

```text
received -> validated -> awaiting_confirmation -> executing -> completed
       \-> rejected                         \-> cancelled
```

Compositor loss, instance termination, or host shutdown cancels non-completed actions. A repeated
payment idempotency key returns its recorded terminal result instead of executing again. The host
retains at most 10,000 terminal payment records until process exit; when full, it rejects new
unique keys but continues serving retained-key replays.

## Opaque Secret Record

Host-private entity; only its random identifier and non-secret readiness metadata may reach a
guest.

| Field | Type | Rules |
| --- | --- | --- |
| handle_id | 256-bit CSPRNG value | Unguessable; redacted from diagnostics/persistence |
| secret_bytes | zeroizing host buffer | Never serialized or exposed to guest |
| owner | PluginPrincipal | Exact principal match required |
| purpose | enum | Specific action and credential kind |
| checkout_session_id | SessionId | Exact session match required |
| created_at/expires_at | monotonic time | Default TTL 120 seconds |
| uses_remaining | u8 | PIN starts at one |
| state | ready/reserved/consumed/expired/revoked | Terminal states cannot resolve |

```text
ready -> reserved -> consumed
  |         |------> revoked
  +-> expired
  +-> revoked
```

Resolution failures use stable non-oracular errors and reveal no existence details.

## Checkout Session

| Field | Type | Rules |
| --- | --- | --- |
| session_id | random host ID | Scoped to principal/instance |
| merchant | verified display identity | Shown by trusted confirmation |
| lines | ordered cart lines | Product, quantity, unit amount, line total |
| subtotal/discount/tax/total | Money | Exact arithmetic and consistent currency |
| state | CheckoutState | Host validates transitions |
| confirmation_snapshot | optional ConfirmedPayment | Immutable amount/currency/merchant snapshot |
| idempotency_key | optional bounded key | Required before payment execution |
| payment_result | optional terminal result | One result per idempotency identity |
| receipt_id | optional ReceiptId | Only after approval |

```text
cart -> ready -> confirming -> payment_pending
                         |-> cancelled -> ready
payment_pending -> approved -> receipted
                -> declined/timeout/unavailable -> recoverable
```

Plugin cart changes after `confirming` do not change the confirmation snapshot.

## Receipt and Print Job

### Receipt

Immutable structured record created only from an approved confirmation snapshot and payment
result. It contains merchant, currency, lines, totals, host time, and non-secret result reference.

### Print job

| Field | Type | Rules |
| --- | --- | --- |
| print_job_id | host ID | One accepted job per request ID |
| receipt_id | ReceiptId | Must reference an approved receipt |
| owner | PluginPrincipal | Must own checkout/receipt |
| preview | host-rendered structure | No raw ESC/POS/device bytes |
| state | accepted/preview_ready/rejected | Simulator only |

## Relationship Summary

```text
PluginBundle -> PluginPrincipal -> PluginInstance
PluginInstance -> UIRegistry -> UI Nodes
PluginInstance -> NavigationStack -> Stack Entries
PluginInstance -> Action Requests -> Checkout Session
Checkout Session -> Opaque Secret Record
Checkout Session -> Payment Result -> Receipt -> Print Job
```
