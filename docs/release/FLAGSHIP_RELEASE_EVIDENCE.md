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

## Prerequisite integration status (2026-09-03)

The prerequisite list is now evidence-backed for the workspace state:

- Integrated with verified seams: tickets 24 (host RBAC employee row scope), 25 (center topology),
  27 (workflow scheduler), 29 (host audit log), 30 (signed update channel), 33 (renderer batch C),
  49 (content collections and typed forms via session-backed `content_editors`), 51 (agent
  conversation UX via the real `conversation_composer`), 53 (plugin/template installation UX via
  tracked `plugin_template_ux` commands), and 55 (project dashboard).
- Ticket 34 (generic POS path certification) stays explicitly scoped to the physical three-station
  baseline run and keeps the `prerequisites` verification gap blocking, so `release_ready` remains
  false.
- The `prerequisites` gap reason is derived from the actual prerequisite statuses, so it can no
  longer drift from the list it summarizes.
