# Flagship restaurant release evidence

Ticket 59 ships a deterministic demo-day proof harness in `studio-flagship`. The harness composes
the host-mediated checkout and `DesignerSession` seams, plus deterministic fixtures for the center,
Stripe PaymentIntent broker, payroll clock, and allowlisted physical peripherals. The payment and
device adapters retain the production boundary: credentials and raw device channels stay in the
host, while the checked-in fixture transport makes the release checks repeatable.

Run the machine-readable report with:

```text
cargo run --locked -p studio-flagship --bin flagship-evidence
```

The report is intentionally `release_ready: false` until the external gates are completed. A
passing deterministic gate means the state machine and adapter contract were exercised; it does
not mean a real printer, physical three-station deployment, or Stripe account was contacted.

The JSON report contains:

- role/PIN authentication with host-only digests;
- center topology, shared check state, offline queueing, duplicate-safe replay, and kitchen replay;
- exact-minute payroll CSV output;
- single, split, per-seat, and optimistic-concurrency billing;
- durable structured receipt and kitchen peripheral jobs with retry/cancel state;
- a declared `/v1/payment_intents` Stripe sandbox route with host-only credential resolution and
  idempotent replay;
- grouped agent authoring through `studio-design::DesignerSession` and named undo;
- hash-chained audit, deterministic digest, recovery, security, accessibility, and capability-matrix
  evidence;
- prerequisite integration status and blocking verification gaps.

`studio-flagship/tests/flagship_release.rs` is the integration-level contract for the report. The
checked-in harness never accepts credentials, raw PINs, raw printer bytes, device paths, or claims
live cloud/hardware proof.
