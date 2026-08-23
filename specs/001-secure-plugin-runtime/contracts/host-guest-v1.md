# Host–Guest Contract v1

## WebAssembly ABI

The module must target `wasm32-unknown-unknown` and export exactly the five entries below. Any
additional compiler, runtime, user, or proposal export is invalid. The AssemblyScript build must
disable runtime export generation. The module imports no WASI functions.

Required exports:

```text
memory
studio_alloc(len: i32) -> i32
studio_dealloc(ptr: i32, len: i32)
studio_init(ptr: i32, len: i32) -> i32
studio_event(ptr: i32, len: i32) -> i32
```

Only allowed import:

```text
studio_host.emit(ptr: i32, len: i32) -> i32
```

All pointer/length pairs use checked unsigned arithmetic against current memory size. The host
copies bytes during the call, validates UTF-8 and the envelope, and retains no guest slice. A
non-zero return is a stable ABI error. `emit` never synchronously re-enters the guest.

## Limits

- One memory, maximum 16 MiB; one bounded table.
- No threads, shared memory, memory64, multi-memory, unknown proposals, or undeclared imports.
- 15 million fuel for module initialization, 10 million fuel for each event call, and a trapping
  50 ms epoch deadline for every guest call.
- Generic message/event 64 KiB; mount 1 MiB; patch 256 KiB/512 operations.
- 5,000 nodes; tree depth 64; node ID 128 UTF-8 bytes; string property 64 KiB.
- 16 pending actions; navigation depth 32.

Any pointer, encoding, feature, limit, import, export, protocol, or trap violation terminates the
instance. The host remains usable and shows its own failure surface.

## Guest Message Envelope

JSON is UTF-8 and uses a closed tagged envelope:

```json
{"type":"mount|patch|navigate|action|log","payload":{}}
```

### Mount

The first successful guest message is exactly one mount:

```json
{
  "type": "mount",
  "payload": {
    "protocol_version": 1,
    "route": "/catalog",
    "root": {"id":"root","kind":"column","props":{},"children":[]}
  }
}
```

The host validates the entire hierarchy before creating or displaying native nodes. Empty or
invalid mounts do not partially render.

### Node kinds

- Layout: `box`, `column`, `row`, `stack`, `grid`, `scroll_view`, `list_view`, `spacer`, `divider`.
- Display: `text`, `icon`, `image`, `card`, `badge`, `progress_indicator`.
- Interaction: `button`, `icon_button`, `checkbox`, `switch`, `slider`, `select`, `text_input`,
  `secret_input`.
- Overlay: `dialog`, `bottom_sheet`, `toast`, `tooltip`.

`container` is an SDK alias that serializes as `box`, not a second wire kind. Properties use a
closed per-kind schema: typed semantic color, spacing, constraints, flex, alignment, border,
radius, shadow, opacity, typography role, visibility, enabled/value state, event bindings, labels,
and approved transition policy. HTML, CSS strings, native class names, raw drawing, shaders, and
device-control data are invalid.

`secret_input` is a host-owned control. Its event may carry only readiness, kind, expiry metadata,
and an opaque reference; it never carries entered text.

### Patch

```json
{
  "type": "patch",
  "payload": {
    "sequence": 7,
    "operations": [
      {"op":"update_prop","node_id":"total","property":"text","value":"$80.00"}
    ]
  }
}
```

Operations are `update_prop`, `insert_child`, `remove_node`, and `replace_node`. Sequence numbers
must increase. The host validates byte/operation budgets, targets, indices, node/property types,
combined resulting uniqueness/depth/count, event ownership, and accessibility constraints before
commit. It applies every operation in order or none. Targeted property updates preserve unrelated
node identity, focus, scroll, input, and interaction state.

### Navigation

Commands are closed tagged values for `push`, `replace`, `pop`, `pop_to`, and `reset`. Routes are
absolute, declared, nested/parameterized, canonicalized, and matched by the host. Invalid routes,
stack overflow, guard denial, and 50 ms guard timeout leave the stack unchanged and produce a
navigation event with a stable error code. Pending payment cannot be abandoned without a host
block or trusted confirmation.

### Action and log

Action envelopes follow [actions-v1.md](actions-v1.md). Guest log text is untrusted, bounded, and
redacted; active handles and raw secrets must never be accepted into durable diagnostics.

## Host Event Envelope

```json
{"type":"ui|navigation|action_result|lifecycle","payload":{}}
```

- UI events are delivered only to the instance owning the source node and contain non-secret
  typed payloads.
- Navigation events include current route, accepted status, and optional stable error code.
- Action results correlate one request and never include raw secrets or active handles.
- Lifecycle events report loading, running, trapped/terminated, or stopped with an optional
  non-sensitive diagnostic.

The host derives event ownership from the active instance and native node registry; no guest
message or event payload contains a caller-selectable owner field. The host queues events and
invokes `studio_event` only after the prior guest call has returned. Delivery is ordered per
instance; terminating an instance discards undelivered events.

## Lifecycle and Recovery

Load/validation failure, trap, resource excess, or protocol violation produces a host-owned error
surface. For a terminated guest, the only milestone recovery is an operator-selected manual
restart. Restart creates a new instance/principal identity and restores no guest memory, UI,
navigation, pending action, handle, or SDK state.

If the Wayland compositor disconnects, the host cancels pending actions, revokes all active
secrets, terminates plugin instances, and exits cleanly without automatic session restoration.

## Stable Error Families

`abi_invalid`, `message_invalid`, `message_too_large`, `protocol_unsupported`, `lifecycle_invalid`,
`tree_invalid`, `patch_invalid`, `sequence_invalid`, `route_not_found`, `navigation_denied`,
`navigation_timeout`, `capability_denied`, `action_invalid`, `resource_exhausted`, and
`guest_terminated`. More specific codes may be added only within a version-compatible closed list.
