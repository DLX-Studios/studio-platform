# Host Action Contract v1

## Common Request

```json
{
  "type": "action",
  "payload": {
    "request_id": "req-123",
    "capability": "payment.simulate",
    "operation": "charge",
    "payload": {}
  }
}
```

The host derives the principal from the calling instance. It validates request size and ID,
manifest declaration, host policy, capability/operation pair, payload schema, session ownership,
resource queues, confirmation, and idempotency before execution. Unknown or undeclared requests
return `capability_denied` without prompting.

Terminal result:

```json
{
  "type": "action_result",
  "payload": {
    "status": "success|failure",
    "request_id": "req-123",
    "payload": {},
    "code": "stable_error_when_failed",
    "message": "non-sensitive diagnostic",
    "retryable": false
  }
}
```

The generated schema uses distinct success/failure shapes; fields irrelevant to a variant are not
present. Results never contain raw credentials or active opaque references.

## Opaque Reference

A `secret_input` readiness event may expose an opaque 256-bit random token representation plus
non-secret kind and expiry metadata. The token:

- is scoped to publisher key, plugin ID, bundle digest, instance ID, purpose, and checkout session;
- expires after 120 seconds by default;
- is single-use for payment authorization;
- cannot be enumerated, resolved, transferred, persisted through Studio, or logged;
- is revoked on expiry, use, instance stop/termination, compositor loss, or host shutdown.

Wrong owner, purpose, session, expiry, reuse, or random token returns the same non-oracular
`authorization_invalid` family without disclosing record existence.

## `payment.simulate / charge`

Request payload:

```json
{
  "checkout_session_id": "checkout-1",
  "amount": {"currency":"USD","minor":8500},
  "authorization_ref": "opaque-value",
  "idempotency_key": "sale-unique-key"
}
```

Before execution, a host-owned surface displays verified publisher/merchant identity, exact
amount, currency, and simulator status. Confirmation creates an immutable snapshot; execution
uses that snapshot even if the guest later changes its cart.

Deterministic outcome by absolute minor-unit amount suffix:

| Suffix | Outcome | Retryable |
| --- | --- | --- |
| `00` | approved | no |
| `01` | declined | no |
| `02` | timeout | yes |
| `03` | terminal unavailable | yes |

Other suffixes use the documented default approved fixture behavior. A terminal payment record is
retained until the host process exits. A repeated idempotency key returns the exact original
terminal result and does not create a second transaction. Reuse with conflicting principal,
session, amount, or currency fails `idempotency_conflict`. The registry holds at most 10,000
terminal records; at capacity, a new unique key fails `idempotency_capacity_exhausted` without
evicting retained records, while retained-key replay remains available.

Approved payload contains a non-secret result reference and host timestamp. Declines/timeouts/
unavailable results contain stable codes and safe retry/return guidance.

## Receipt

An approval permits the host to create a structured receipt from the confirmation snapshot and
result, containing merchant, ordered lines, quantities, integer unit/line totals, subtotal,
discount, tax, total, currency, result reference, and host time. Values must agree exactly with
the approved snapshot. Raw secret and opaque reference fields do not exist in the schema.

## `printer.simulate / preview`

Request payload identifies or embeds the authorized structured receipt according to the generated
schema. The host validates ownership and approval, creates at most one in-memory job per accepted
request, and displays a host-owned preview. Raw ESC/POS, arbitrary bytes, device names, paths, and
network destinations are rejected.

## Cancellation and Recovery

Plugin termination, compositor loss, or host shutdown cancels pending non-terminal requests and
revokes related secrets. Payment retries require the documented recoverable state and either the
same idempotency identity for result retrieval or a new valid authorization flow when the prior
handle has been consumed/revoked.

Stable errors include `capability_denied`, `action_invalid`, `queue_full`,
`authorization_required`, `authorization_invalid`, `confirmation_cancelled`,
`idempotency_conflict`, `idempotency_capacity_exhausted`, `payment_declined`, `payment_timeout`, `terminal_unavailable`,
`receipt_invalid`, `print_duplicate`, and `action_cancelled`.
