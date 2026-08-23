# Studio Milestone-One Threat Model

## Protected assets

- User PINs and future payment credentials.
- Merchant identity and transaction amounts.
- Host filesystem, environment, network, and hardware.
- Plugin signing trust store.
- Availability and responsiveness of the native host.
- Data belonging to other plugin instances.

## Adversary

Assume a plugin bundle may be malicious, compromised, malformed, or designed to exhaust CPU, memory, UI resources, and host queues. Assume it can inspect and modify every byte in its own linear memory and can deliberately invoke ABI functions with invalid values.

Milestone one does not attempt to protect against a compromised host process, kernel, compositor, administrator account, or signing authority.

## Required controls

- Verify bundles before compilation.
- Link no WASI and accept only the documented import/export surface.
- Apply fuel, epoch, memory, table, message, tree, and queue limits.
- Copy and bounds-check guest memory during each call; retain no guest slices.
- Fully validate messages before mutating native state.
- Bind permissions and handles to a signed plugin principal and instance.
- Keep secrets in host memory and erase them after use or expiry.
- Keep confirmations and error surfaces host-owned.
- Redact secrets and active handles from telemetry and diagnostics.
- Treat every capability request as untrusted input, including requests from signed plugins.

## Initial capability boundary

Only deterministic payment and printer simulators exist. There is no generic network, filesystem, clipboard, media, child-process, child-module, or hardware API.

## Trust and failure flows

Production admission validates the bounded deterministic archive and closed manifest, resolves an
enabled publisher key, verifies the raw Ed25519 signature over the RFC 8785 digest document, and
only then compiles/instantiates WebAssembly. Development mode is an explicit separate CLI path and
keeps a host-owned unsigned-bundle warning visible.

A trap, fuel/epoch/resource excess, ABI violation, or malformed post-mount message permanently
terminates that instance. Studio cancels its pending actions, revokes/zeroizes its opaque records,
drops UI/navigation/plugin-local state, and shows a trusted failure surface. No automatic restart
exists. A manual restart creates a new instance identity and restores no plugin state or handles.

Native Wayland compositor loss is process-terminal. Cleanup order is: cancel actions, revoke
secrets, terminate guests, close retained/native/navigation state, then request process exit. The
old state is never attached to a replacement compositor.

## Opaque-reference lifecycle

Sensitive input is captured in a Studio-owned native control. A random 256-bit reference is scoped
to the exact publisher key, plugin ID, bundle digest, fresh instance, purpose, and checkout session.
Payment PIN references expire after 120 seconds and are single-use. Expired, absent, consumed,
revoked, wrong-purpose, wrong-session, and wrong-owner resolution all return the same non-oracular
error. Termination and compositor loss revoke outstanding records; raw bytes and active references
are rejected or redacted from observable and persistent artifacts.

## Simulator boundaries

`payment.simulate/charge` and `printer.simulate/preview` are the only milestone capabilities.
Payment has no provider/network implementation and derives its documented result deterministically
from exact confirmed integer money. Idempotency records are bounded and never evicted. Printing
accepts a structured approved receipt identity only; raw ESC/POS, device paths, arbitrary bytes,
and network destinations are not representable in the accepted request schema.

## Residual risks

- A compromised host process, kernel, compositor, administrator, signing key, compiler, or GPU
  driver is outside the sandbox guarantee.
- Side channels such as timing, screen observation, accessibility-service compromise, memory
  pressure, and denial of service are reduced but not eliminated.
- Native dependency vulnerabilities can affect the host despite guest isolation; lockfile and
  release dependency audits are mandatory.
- Simulators must not be mistaken for production payment certification or hardware integration.
- Manual native accessibility checks remain necessary for visual focus, scaling, and assistive
  technology behavior.
