# Live payments and peripherals remediation

The flagship host now exposes two bounded adapter seams:

- `StripeSandboxAdapter` creates and confirms a PaymentIntent through the host-owned REST broker
  route `/v1/payment_intents`. The broker resolves the protected test credential at send time,
  keeps it out of adapter results/audit records, and retains the provider idempotency key. The
  adapter enforces positive minor-unit amounts, `usd`, a five-second timeout ceiling, and at most
  two retries for timeout/provider-unavailable failures. Declines, malformed requests, route
  denial, and idempotency conflicts are terminal and classified separately.
- `DurablePeripheralAdapters` discovers only the approved receipt-printer and kitchen-display
  families. Structured jobs are retained across state transitions (`accepted`, `printing`,
  `succeeded`, `failed`, `cancelled`) and expose retry/cancel operations without exposing a path,
  file descriptor, raw device handle, or printer byte stream.

`crates/studio-flagship/fixtures/live-payments-peripherals.json` is the deterministic source
fixture for route, amount, allowlist, lifecycle, retry, and redaction assertions. It intentionally
contains no Stripe credential or hardware identifier beyond the stable host-owned device IDs.
The fixture proves adapter behavior; release certification still requires the operator to run the
same journey against the approved Stripe sandbox and physical baseline hardware.
